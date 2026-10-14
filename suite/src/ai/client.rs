//! Talking to the providers: their OpenAI-compatible chat/completions and models endpoints, and
//! what an error means for switching channels (a limit, a bad key, a missing model, an outage).
//! Each key goes only to its own provider (see `Provider::base_url`).

use std::time::Duration;

use serde_json::{Value, json};

use super::channels::{DAY, Provider, now};

#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// Too many requests for now: rest until `until` (Retry-After, a reset header, or a minute).
    Busy { until: u64 },
    /// A daily limit, quota or used-up credit: rest until it resets.
    Daily { until: u64 },
    /// The key was refused.
    BadKey,
    /// This model can't answer (not found, not available, down): try the next one.
    Model,
    /// The provider or the network failed; nothing to rest for.
    Other,
}

#[derive(Clone, Debug)]
pub struct Error {
    pub failure: Failure,
    /// Short and readable, for the answer's note and the channel's status.
    pub message: String,
}

pub struct Message {
    pub role: &'static str,
    pub content: String,
}

fn agent() -> &'static ureq::Agent {
    static AGENT: std::sync::OnceLock<ureq::Agent> = std::sync::OnceLock::new();
    AGENT.get_or_init(|| {
        let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
        ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_global(Some(Duration::from_secs(120)))
            .timeout_connect(Some(Duration::from_secs(15)))
            .user_agent(concat!("CraftSuite/", env!("CARGO_PKG_VERSION")))
            .tls_config(tls)
            .build()
            .into()
    })
}

struct Reply {
    status: u16,
    body: String,
    retry_after: Option<u64>,
    reset: Option<u64>,
}

fn header(r: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    r.headers().get(name).and_then(|v| v.to_str().ok()).map(str::to_string)
}

/// A reset time from X-RateLimit-Reset: milliseconds or seconds since 1970, or seconds from now.
fn reset_time(v: &str, now: u64) -> Option<u64> {
    let n: f64 = v.trim().parse().ok()?;
    let t = if n > 1e12 { (n / 1000.0) as u64 } else if n > 1e9 { n as u64 } else { now + n.ceil() as u64 };
    (t > now).then_some(t)
}

fn send(provider: Provider, key: &str, method: &str, path: &str, body: Option<&Value>) -> Result<Reply, Error> {
    let url = format!("{}{}", provider.base_url(), path);
    let auth = format!("Bearer {key}");
    let result = if method == "POST" {
        let mut req = agent().post(&url).header("Authorization", &auth).header("Content-Type", "application/json");
        if provider == Provider::OpenRouter {
            req = req.header("X-OpenRouter-Title", "Craft Suite");
        }
        req.send(serde_json::to_vec(body.unwrap_or(&json!({}))).unwrap_or_default())
    } else {
        let mut req = agent().get(&url);
        if !key.is_empty() {
            req = req.header("Authorization", &auth);
        }
        req.call()
    };
    let mut resp = result.map_err(|e| Error { failure: Failure::Other, message: format!("couldn't reach {}: {}", provider.label(), plain_error(&e)) })?;
    let now = now();
    let retry_after = header(&resp, "retry-after").and_then(|v| v.trim().parse::<f64>().ok()).map(|s| now + s.ceil().max(1.0) as u64);
    let reset = header(&resp, "x-ratelimit-reset").and_then(|v| reset_time(&v, now));
    let status = resp.status().as_u16();
    let body = resp.body_mut().with_config().limit(16 * 1024 * 1024).read_to_string().unwrap_or_default();
    Ok(Reply { status, body, retry_after, reset })
}

fn plain_error(e: &ureq::Error) -> String {
    match e {
        ureq::Error::Timeout(_) => "it took too long to answer".into(),
        ureq::Error::HostNotFound => "no internet connection, or the address wasn't found".into(),
        ureq::Error::Io(io) => io.to_string(),
        other => other.to_string(),
    }
}

/// The error message in a provider's body: {error:{message}} or Gemini's [{error:{…}}].
fn body_message(body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let err = match &v {
        Value::Array(a) => a.first().and_then(|x| x.get("error")).cloned(),
        _ => v.get("error").cloned(),
    };
    let msg = match err {
        Some(Value::String(s)) => s,
        Some(e) => {
            let m = e.get("message").and_then(Value::as_str).unwrap_or_default().to_string();
            let raw = e.pointer("/metadata/raw").and_then(Value::as_str).unwrap_or_default();
            if raw.is_empty() { m } else { format!("{m} ({raw})") }
        }
        None => body.chars().take(200).collect(),
    };
    let msg = msg.trim().replace('\n', " ");
    if msg.chars().count() > 240 { format!("{}…", msg.chars().take(240).collect::<String>()) } else { msg }
}

/// Whether a 429's text points at a daily (or longer) limit rather than a per-minute one.
fn says_daily(text: &str) -> bool {
    let t = text.to_ascii_lowercase();
    ["per day", "per-day", "perday", "daily", "requests per day", "(rpd)", "(tpd)", "free-models-per-day", "quota_exceeded", "per month", "monthly", "quota exceeded"]
        .iter()
        .any(|k| t.contains(k))
}

/// What an unsuccessful reply means.
fn classify(provider: Provider, key: &str, reply: &Reply) -> Error {
    let now = now();
    let msg = body_message(&reply.body);
    let short = |what: &str| if msg.is_empty() { what.to_string() } else { format!("{what}: {msg}") };
    let daily = || Failure::Daily { until: reply.reset.filter(|t| *t > now + 3600).unwrap_or_else(|| provider.next_daily_reset(now)) };
    match reply.status {
        401 => Error { failure: Failure::BadKey, message: short("key refused") },
        403 if provider != Provider::OpenRouter => Error { failure: Failure::BadKey, message: short("key not allowed") },
        402 => Error { failure: daily(), message: short("out of credit or quota") },
        429 => {
            // OpenRouter: an upstream provider limited this model (try another), or OpenRouter's
            // own limit, which the key's free-request count tells apart.
            if provider == Provider::OpenRouter {
                let v: Value = serde_json::from_str(&reply.body).unwrap_or(Value::Null);
                if v.pointer("/error/metadata/provider_name").is_some() || v.pointer("/error/metadata/provider_code").is_some() {
                    return Error { failure: Failure::Model, message: short("this free model is busy upstream") };
                }
                if says_daily(&msg) || openrouter_free_left(key) == Some(0) {
                    return Error { failure: daily(), message: short("daily free limit reached") };
                }
            } else if says_daily(&msg) {
                return Error { failure: daily(), message: short("daily limit reached") };
            }
            let until = reply.retry_after.or(reply.reset).unwrap_or(now + 60).min(now + DAY);
            if until > now + 6 * 3600 {
                Error { failure: Failure::Daily { until }, message: short("limit reached") }
            } else {
                Error { failure: Failure::Busy { until }, message: short("rate limit") }
            }
        }
        400 | 404 | 422 | 403 | 413 => Error { failure: Failure::Model, message: short(&format!("error {}", reply.status)) },
        s => Error { failure: if s >= 500 || s == 408 || s == 498 { Failure::Model } else { Failure::Other }, message: short(&format!("error {s}")) },
    }
}

/// OpenRouter: how many free-model requests the key has left today, if it says.
fn openrouter_free_left(key: &str) -> Option<u64> {
    let r = send(Provider::OpenRouter, key, "GET", "/key", None).ok()?;
    let v: Value = serde_json::from_str(&r.body).ok()?;
    v.pointer("/data/free_model_daily_requests/remaining").and_then(Value::as_u64)
}

pub struct Answer {
    pub text: String,
    /// The model the provider says answered (OpenRouter may name the exact one).
    pub model: String,
}

pub fn chat(provider: Provider, key: &str, model: &str, messages: &[Message]) -> Result<Answer, Error> {
    let msgs: Vec<Value> = messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect();
    let body = json!({"model": model, "messages": msgs, "temperature": 0.3, "stream": false});
    let reply = send(provider, key, "POST", "/chat/completions", Some(&body))?;
    if !(200..300).contains(&reply.status) {
        return Err(classify(provider, key, &reply));
    }
    let v: Value = serde_json::from_str(&reply.body)
        .map_err(|_| Error { failure: Failure::Model, message: format!("{} sent something that isn't an answer", provider.label()) })?;
    // An error inside a 200 (OpenRouter does this for some upstream failures).
    if v.get("error").is_some() {
        let code = v.pointer("/error/code").and_then(Value::as_u64).unwrap_or(500) as u16;
        return Err(classify(provider, key, &Reply { status: code, body: reply.body, retry_after: reply.retry_after, reset: reply.reset }));
    }
    let text = v.pointer("/choices/0/message/content").and_then(Value::as_str).unwrap_or_default().trim().to_string();
    if text.is_empty() {
        return Err(Error { failure: Failure::Model, message: "the model sent an empty answer".into() });
    }
    let model = v.get("model").and_then(Value::as_str).unwrap_or(model).to_string();
    Ok(Answer { text, model })
}

#[derive(Clone, Debug)]
pub struct ModelInfo {
    pub id: String,
    /// OpenRouter: free to use.
    pub free: bool,
    /// For ranking: OpenRouter's intelligence index when listed.
    pub intelligence: Option<f64>,
    pub created: u64,
}

/// The provider's models that can chat, as listed live.
pub fn list_models(provider: Provider, key: &str) -> Result<Vec<ModelInfo>, Error> {
    let reply = send(provider, key, "GET", "/models", None)?;
    if !(200..300).contains(&reply.status) {
        return Err(classify(provider, key, &reply));
    }
    let v: Value = serde_json::from_str(&reply.body).map_err(|_| Error { failure: Failure::Other, message: "the model list couldn't be read".into() })?;
    let empty = Vec::new();
    let data = v.get("data").and_then(Value::as_array).or_else(|| v.as_array()).unwrap_or(&empty);
    let mut out = Vec::new();
    for m in data {
        let Some(id) = m.get("id").and_then(Value::as_str) else { continue };
        let id = id.strip_prefix("models/").unwrap_or(id).to_string();
        // Not for chat: speech, embeddings, images, moderation; inactive or archived models.
        let lower = id.to_ascii_lowercase();
        if ["whisper", "embed", "tts", "moderation", "guard", "imagen", "veo", "-ocr", "transcribe", "orpheus", "playai"].iter().any(|k| lower.contains(k)) {
            continue;
        }
        if m.get("active") == Some(&Value::Bool(false)) || m.get("archived") == Some(&Value::Bool(true)) {
            continue;
        }
        if m.pointer("/capabilities/completion_chat") == Some(&Value::Bool(false)) {
            continue;
        }
        let price = |k: &str| m.pointer(&format!("/pricing/{k}")).and_then(Value::as_str).and_then(|p| p.parse::<f64>().ok());
        let free = id.ends_with(":free") || (price("prompt") == Some(0.0) && price("completion") == Some(0.0));
        let intelligence = m
            .pointer("/benchmarks/artificial_analysis/intelligence_index")
            .or_else(|| m.pointer("/benchmarks/artificial_analysis/intelligence"))
            .and_then(Value::as_f64);
        let created = m.get("created").and_then(Value::as_u64).unwrap_or(0);
        // Text output only (OpenRouter lists modalities).
        if let Some(out_mods) = m.pointer("/architecture/output_modalities").and_then(Value::as_array)
            && !out_mods.iter().any(|x| x.as_str() == Some("text"))
        {
            continue;
        }
        out.push(ModelInfo { id, free, intelligence, created });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

/// Billions of parameters in a model's name ("llama-3.3-70b-instruct" → 70), for ranking.
fn size_in_name(id: &str) -> Option<f64> {
    let lower = id.to_ascii_lowercase();
    let mut best: Option<f64> = None;
    for part in lower.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.')) {
        // "70b", "8x7b" (mixture: count the total), skipping "a3b" (active parameters).
        if part.starts_with('a') {
            continue;
        }
        if let Some(num) = part.strip_suffix('b') {
            let v = match num.split_once('x') {
                Some((n, each)) => n.parse::<f64>().ok().zip(each.parse::<f64>().ok()).map(|(n, e)| n * e),
                None => num.parse::<f64>().ok(),
            };
            if let Some(v) = v {
                best = Some(best.map_or(v, |b: f64| b.max(v)));
            }
        }
    }
    best
}

/// OpenRouter's free models, strongest first: by its intelligence index where listed, then by
/// the size in the name, then the newest. A guess at "strongest", not a measurement.
pub fn rank_free(models: &[ModelInfo]) -> Vec<String> {
    let mut free: Vec<&ModelInfo> = models.iter().filter(|m| m.free && m.id != "openrouter/auto" && !m.id.starts_with("openrouter/")).collect();
    free.sort_by(|a, b| {
        let key = |m: &ModelInfo| (m.intelligence.unwrap_or(-1.0), size_in_name(&m.id).unwrap_or(0.0), m.created as f64);
        let (ka, kb) = (key(a), key(b));
        kb.0.total_cmp(&ka.0).then(kb.1.total_cmp(&ka.1)).then(kb.2.total_cmp(&ka.2))
    });
    free.into_iter().map(|m| m.id.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(status: u16, body: &str) -> Reply {
        Reply { status, body: body.into(), retry_after: None, reset: None }
    }

    #[test]
    fn reads_error_bodies() {
        assert_eq!(body_message(r#"{"error":{"message":"bad"}}"#), "bad");
        assert_eq!(body_message(r#"[{"error":{"code":429,"message":"Resource has been exhausted","status":"RESOURCE_EXHAUSTED"}}]"#), "Resource has been exhausted");
    }

    #[test]
    fn classifies() {
        let k = "";
        assert!(matches!(classify(Provider::Groq, k, &reply(401, "{}")).failure, Failure::BadKey));
        assert!(matches!(classify(Provider::Mistral, k, &reply(404, "{}")).failure, Failure::Model));
        assert!(matches!(classify(Provider::Groq, k, &reply(503, "{}")).failure, Failure::Model));
        let busy = classify(Provider::Groq, k, &reply(429, r#"{"error":{"message":"Rate limit reached on requests per minute (RPM)"}}"#));
        assert!(matches!(busy.failure, Failure::Busy { until } if until >= now() + 59 && until <= now() + 61));
        let daily = classify(Provider::Groq, k, &reply(429, r#"{"error":{"message":"Rate limit reached on requests per day (RPD)"}}"#));
        assert!(matches!(daily.failure, Failure::Daily { until } if until == (now() / DAY + 1) * DAY));
        let gem = classify(Provider::Gemini, k, &reply(429, r#"{"error":{"code":"quota_exceeded","message":"Quota exceeded"}}"#));
        assert!(matches!(gem.failure, Failure::Daily { until } if until == Provider::Gemini.next_daily_reset(now())));
        let mut r = reply(429, "{}");
        r.retry_after = Some(now() + 17);
        assert!(matches!(classify(Provider::Mistral, k, &r).failure, Failure::Busy { until } if until == now() + 17));
        assert!(matches!(classify(Provider::OpenRouter, k, &reply(429, r#"{"error":{"message":"Rate limit exceeded: free-models-per-day"}}"#)).failure, Failure::Daily { .. }));
        assert!(matches!(
            classify(Provider::OpenRouter, k, &reply(429, r#"{"error":{"message":"x is temporarily rate-limited upstream","metadata":{"provider_name":"Chutes"}}}"#)).failure,
            Failure::Model
        ));
    }

    #[test]
    fn reset_headers() {
        let now = 1_800_000_000;
        assert_eq!(reset_time("1800000060000", now), Some(1_800_000_060));
        assert_eq!(reset_time("1800000060", now), Some(1_800_000_060));
        assert_eq!(reset_time("30", now), Some(now + 30));
    }

    #[test]
    fn ranks_free_models() {
        let m = |id: &str, free: bool, intel: Option<f64>, created: u64| ModelInfo { id: id.into(), free, intelligence: intel, created };
        let models = vec![
            m("small/model-8b:free", true, None, 3),
            m("big/model-70b:free", true, None, 1),
            m("paid/model-405b", false, None, 1),
            m("smart/x:free", true, Some(60.0), 0),
            m("moe/qwen3-235b-a22b:free", true, None, 0),
        ];
        assert_eq!(rank_free(&models), vec!["smart/x:free", "moe/qwen3-235b-a22b:free", "big/model-70b:free", "small/model-8b:free"]);
        assert_eq!(size_in_name("mistralai/mixtral-8x7b"), Some(56.0));
    }
}
