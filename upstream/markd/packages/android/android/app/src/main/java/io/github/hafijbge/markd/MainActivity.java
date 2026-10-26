package io.github.hafijbge.markd;

import android.app.Activity;
import android.content.Intent;
import android.graphics.Color;
import android.graphics.Insets;
import android.net.Uri;
import android.os.Build;
import android.os.Bundle;
import android.util.Log;
import android.view.View;
import android.view.Window;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.webkit.ConsoleMessage;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.LinearLayout;

import androidx.webkit.WebViewCompat;
import androidx.webkit.WebViewFeature;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.Collections;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

/**
 * Hosts the web editor (packages/android/src, built into assets/www) in a
 * WebView. The page is served from https://appassets.androidplatform.net:
 * /assets/... from the APK assets, /doc/<token>/<path> from the folder of the
 * open document (so relative image paths in the Markdown resolve).
 */
public class MainActivity extends Activity {

    static final String HOST = "appassets.androidplatform.net";
    static final String ORIGIN = "https://" + HOST;
    private static final String START_URL = ORIGIN + "/assets/www/index.html";
    private static final String TAG = "Markd";

    private static final Map<String, String> MIME = new HashMap<>();

    static {
        MIME.put("html", "text/html");
        MIME.put("js", "text/javascript");
        MIME.put("mjs", "text/javascript");
        MIME.put("css", "text/css");
        MIME.put("json", "application/json");
        MIME.put("svg", "image/svg+xml");
        MIME.put("png", "image/png");
        MIME.put("jpg", "image/jpeg");
        MIME.put("jpeg", "image/jpeg");
        MIME.put("gif", "image/gif");
        MIME.put("webp", "image/webp");
        MIME.put("bmp", "image/bmp");
        MIME.put("ico", "image/x-icon");
        MIME.put("woff", "font/woff");
        MIME.put("woff2", "font/woff2");
        MIME.put("ttf", "font/ttf");
        MIME.put("otf", "font/otf");
        MIME.put("wasm", "application/wasm");
        MIME.put("txt", "text/plain");
        MIME.put("md", "text/markdown");
    }

    private WebView webView;
    private View topInset;
    private View bottomInset;
    private DocStore store;
    private Bridge bridge;
    private Uri launchUri;
    private volatile Uri currentDocument;
    private volatile DocStore.Folder currentFolder;
    /** Ask for folder access at most once per document when an image cannot be served. */
    private volatile boolean folderRequested;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        store = new DocStore(getContentResolver());
        bridge = new Bridge(this, store);
        launchUri = documentFrom(getIntent());

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        topInset = new View(this);
        bottomInset = new View(this);
        webView = new WebView(this);
        int brand = Color.parseColor("#185ABD");
        topInset.setBackgroundColor(brand);
        bottomInset.setBackgroundColor(brand);
        root.addView(topInset, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0));
        root.addView(webView, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f));
        root.addView(bottomInset, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0));
        setContentView(root);
        setupEdgeToEdge(root);
        setupWebView();
        // Unsaved edits survive process death through the page's own draft storage.
        webView.loadUrl(START_URL);
    }

    /** System bars are drawn over coloured spacer views sized from the window insets. */
    private void setupEdgeToEdge(LinearLayout root) {
        Window window = getWindow();
        if (Build.VERSION.SDK_INT < 30) return;
        window.setDecorFitsSystemWindows(false);
        window.setNavigationBarContrastEnforced(false);
        root.setOnApplyWindowInsetsListener((view, insets) -> {
            Insets bars = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            Insets ime = insets.getInsets(WindowInsets.Type.ime());
            topInset.getLayoutParams().height = bars.top;
            bottomInset.getLayoutParams().height = Math.max(bars.bottom, ime.bottom);
            view.setPadding(bars.left, 0, bars.right, 0);
            topInset.requestLayout();
            bottomInset.requestLayout();
            return WindowInsets.CONSUMED;
        });
    }

    private void setupWebView() {
        WebView.setWebContentsDebuggingEnabled(BuildConfig.DEBUG);
        WebSettings settings = webView.getSettings();
        settings.setJavaScriptEnabled(true);
        settings.setDomStorageEnabled(true);
        settings.setAllowFileAccess(false);
        settings.setAllowContentAccess(false);
        settings.setSupportZoom(false);
        settings.setBuiltInZoomControls(false);
        settings.setTextZoom(100);
        settings.setMediaPlaybackRequiresUserGesture(true);
        webView.setOverScrollMode(View.OVER_SCROLL_NEVER);
        setHandwriting(true);

        if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
            WebViewCompat.addWebMessageListener(webView, "MarkdHost", Collections.singleton(ORIGIN), bridge);
        } else {
            Log.e(TAG, "WebView too old: WEB_MESSAGE_LISTENER unsupported; file access disabled");
        }

        webView.setWebChromeClient(new WebChromeClient() {
            @Override
            public boolean onConsoleMessage(ConsoleMessage message) {
                Log.d(TAG, message.message() + " (" + message.sourceId() + ":" + message.lineNumber() + ")");
                return true;
            }
        });
        webView.setWebViewClient(new WebViewClient() {
            @Override
            public WebResourceResponse shouldInterceptRequest(WebView view, WebResourceRequest request) {
                Uri url = request.getUrl();
                if (!HOST.equals(url.getHost())) return null;
                String path = url.getPath() == null ? "" : url.getPath();
                if (path.startsWith("/assets/")) return serveAsset(path.substring("/assets/".length()));
                if (path.startsWith("/doc/")) return serveDocumentFile(path.substring("/doc/".length()));
                return notFound();
            }

            @Override
            public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
                Uri url = request.getUrl();
                if (HOST.equals(url.getHost())) return false;
                String scheme = url.getScheme();
                if ("http".equals(scheme) || "https".equals(scheme) || "mailto".equals(scheme)) {
                    try {
                        startActivity(new Intent(Intent.ACTION_VIEW, url));
                    } catch (Exception ignored) {
                        // No browser installed.
                    }
                }
                return true;
            }
        });
    }

    private static String mimeOf(String path) {
        int dot = path.lastIndexOf('.');
        String ext = dot >= 0 ? path.substring(dot + 1).toLowerCase(Locale.ROOT) : "";
        String mime = MIME.get(ext);
        return mime == null ? "application/octet-stream" : mime;
    }

    private static WebResourceResponse notFound() {
        return new WebResourceResponse("text/plain", "UTF-8", 404, "Not Found",
            Collections.emptyMap(), new ByteArrayInputStream(new byte[0]));
    }

    private WebResourceResponse serveAsset(String path) {
        if (path.contains("..")) return notFound();
        try {
            InputStream in = getAssets().open(path);
            return new WebResourceResponse(mimeOf(path), null, in);
        } catch (IOException e) {
            return notFound();
        }
    }

    /** {@code tokenAndPath} is "<doc token>/<relative path>"; the token only busts caches. */
    private WebResourceResponse serveDocumentFile(String tokenAndPath) {
        int slash = tokenAndPath.indexOf('/');
        if (slash < 0) return notFound();
        String relative = tokenAndPath.substring(slash + 1);
        DocStore.Folder folder = currentFolder;
        if (folder == null && currentDocument != null) {
            folder = store.folderOf(currentDocument);
            currentFolder = folder;
        }
        if (folder == null) {
            requestFolderOnce();
            return notFound();
        }
        try {
            Uri file = store.resolve(folder, relative);
            if (file == null) return notFound();
            InputStream in = getContentResolver().openInputStream(file);
            if (in == null) return notFound();
            Map<String, String> headers = new HashMap<>();
            headers.put("Cache-Control", "no-cache");
            return new WebResourceResponse(mimeOf(relative), null, 200, "OK", headers, in);
        } catch (Exception e) {
            Log.w(TAG, "Cannot serve " + relative, e);
            return notFound();
        }
    }

    private void requestFolderOnce() {
        if (folderRequested || currentDocument == null) return;
        folderRequested = true;
        runOnUiThread(() -> sendEvent("folderNeeded", "null", null));
    }

    void setCurrentDocument(Uri uri) {
        currentDocument = uri;
        currentFolder = null;
        folderRequested = false;
        // Shown in the home screen's Recent row.
        if (uri != null && "content".equals(uri.getScheme())) {
            com.hafij.inkwell.Recents.add(this, uri.toString(), store.displayName(uri));
        }
    }

    /** Back to the home screen; it is normally just below this activity. */
    void openHome() {
        startActivity(new Intent(this, com.hafij.inkwell.HomeActivity.class)
            .addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP | Intent.FLAG_ACTIVITY_SINGLE_TOP));
    }

    void onFolderGranted() {
        currentFolder = null;
    }

    Uri takeLaunchUri() {
        Uri uri = launchUri;
        launchUri = null;
        return uri;
    }

    void setHandwriting(boolean enabled) {
        if (Build.VERSION.SDK_INT >= 33) runOnUiThread(() -> webView.setAutoHandwritingEnabled(enabled));
    }

    void setChrome(String top, String bottom, boolean dark) {
        runOnUiThread(() -> {
            int topColor = parseColor(top, Color.parseColor("#185ABD"));
            int bottomColor = parseColor(bottom, topColor);
            topInset.setBackgroundColor(topColor);
            bottomInset.setBackgroundColor(bottomColor);
            Window window = getWindow();
            if (Build.VERSION.SDK_INT < 35) {
                window.setStatusBarColor(topColor);
                window.setNavigationBarColor(bottomColor);
            }
            // Both bars sit on dark (blue / charcoal) colours: always light icons.
            if (Build.VERSION.SDK_INT >= 30) {
                WindowInsetsController controller = window.getInsetsController();
                if (controller != null) {
                    controller.setSystemBarsAppearance(0,
                        WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS);
                }
            }
        });
    }

    private static int parseColor(String value, int fallback) {
        try {
            return value == null || value.isEmpty() ? fallback : Color.parseColor(value);
        } catch (IllegalArgumentException e) {
            return fallback;
        }
    }

    private void sendEvent(String name, String payloadJson, android.webkit.ValueCallback<String> callback) {
        String script = "window.__markdNative ? window.__markdNative.event(" + org.json.JSONObject.quote(name) + ", " + payloadJson + ") : null";
        webView.evaluateJavascript(script, callback);
    }

    /** Document uri carried by a VIEW / EDIT / SEND intent, with its grant kept if possible. */
    private Uri documentFrom(Intent intent) {
        if (intent == null) return null;
        Uri uri = null;
        String action = intent.getAction();
        if (Intent.ACTION_VIEW.equals(action) || Intent.ACTION_EDIT.equals(action)) uri = intent.getData();
        if (uri == null || !"content".equals(uri.getScheme())) return null;
        store.persist(uri, intent.getFlags());
        return uri;
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        Uri uri = documentFrom(intent);
        if (uri != null) sendEvent("open", "{\"uri\":" + org.json.JSONObject.quote(uri.toString()) + "}", null);
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        if (!bridge.onActivityResult(requestCode, resultCode, data)) super.onActivityResult(requestCode, resultCode, data);
    }

    @Override
    protected void onPause() {
        sendEvent("pause", "null", null);
        super.onPause();
    }

    @Override
    @SuppressWarnings("deprecation")
    public void onBackPressed() {
        sendEvent("back", "null", value -> {
            if (!"true".equals(value)) super.onBackPressed();
        });
    }

    @Override
    protected void onDestroy() {
        if (webView != null) webView.destroy();
        super.onDestroy();
    }
}
