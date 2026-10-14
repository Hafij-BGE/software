package io.github.hafijbge.craftsuite;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.Intent;
import android.net.Uri;
import android.os.Bundle;
import android.util.Base64;
import android.webkit.JavascriptInterface;
import android.webkit.ValueCallback;
import android.webkit.WebChromeClient;
import android.webkit.WebResourceRequest;
import android.webkit.WebResourceResponse;
import android.webkit.WebSettings;
import android.webkit.WebView;
import android.webkit.WebViewClient;
import android.widget.Toast;

import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.HashMap;
import java.util.Map;

/**
 * Craft Suite for Android: the launcher page and WordCraft, DeckCraft and GridCraft's web versions
 * (WebAssembly), shipped in the APK's assets and shown in a WebView.
 *
 * The pages load from https://appassets.androidplatform.net/, served from the assets here (a
 * secure origin, with the right content types for .wasm). Opening a file uses the system's file
 * picker; saving (the web apps' downloads, passed on by android-shim.js) asks where to save
 * through the system's Create document screen.
 */
public class MainActivity extends Activity {
    private static final String HOST = "appassets.androidplatform.net";
    private static final String HOME = "https://" + HOST + "/index.html";
    private static final int REQUEST_OPEN = 1;
    private static final int REQUEST_SAVE = 2;

    private static final Map<String, String> TYPES = new HashMap<>();
    static {
        TYPES.put("html", "text/html");
        TYPES.put("js", "text/javascript");
        TYPES.put("mjs", "text/javascript");
        TYPES.put("wasm", "application/wasm");
        TYPES.put("css", "text/css");
        TYPES.put("json", "application/json");
        TYPES.put("png", "image/png");
        TYPES.put("svg", "image/svg+xml");
        TYPES.put("ico", "image/x-icon");
        TYPES.put("ttf", "font/ttf");
        TYPES.put("otf", "font/otf");
        TYPES.put("woff2", "font/woff2");
        TYPES.put("txt", "text/plain");
    }

    private WebView web;
    private ValueCallback<Uri[]> pendingOpen;
    private byte[] pendingSave;

    @Override
    protected void onCreate(Bundle saved) {
        super.onCreate(saved);
        web = new WebView(this);
        WebSettings s = web.getSettings();
        s.setJavaScriptEnabled(true);
        s.setDomStorageEnabled(true);
        s.setAllowFileAccess(false);
        s.setAllowContentAccess(false);
        s.setMediaPlaybackRequiresUserGesture(false);
        s.setSupportZoom(false);
        web.addJavascriptInterface(new Bridge(), "CraftSuiteAndroid");
        web.setWebViewClient(new WebViewClient() {
            @Override
            public WebResourceResponse shouldInterceptRequest(WebView view, WebResourceRequest request) {
                Uri url = request.getUrl();
                if (!HOST.equals(url.getHost())) {
                    return null;
                }
                return asset(url.getPath());
            }

            @Override
            public boolean shouldOverrideUrlLoading(WebView view, WebResourceRequest request) {
                Uri url = request.getUrl();
                if (HOST.equals(url.getHost())) {
                    return false;
                }
                // Links out of the suite (help pages, the apps' websites) open in the browser.
                try {
                    startActivity(new Intent(Intent.ACTION_VIEW, url));
                } catch (ActivityNotFoundException e) {
                    toast("No app can open " + url);
                }
                return true;
            }
        });
        web.setWebChromeClient(new WebChromeClient() {
            @Override
            public boolean onShowFileChooser(WebView view, ValueCallback<Uri[]> callback, FileChooserParams params) {
                if (pendingOpen != null) {
                    pendingOpen.onReceiveValue(null);
                }
                pendingOpen = callback;
                Intent intent = params.createIntent();
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                // Office files are often typed as application/octet-stream: offer every file.
                intent.setType("*/*");
                if (params.getMode() == FileChooserParams.MODE_OPEN_MULTIPLE) {
                    intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true);
                }
                try {
                    startActivityForResult(intent, REQUEST_OPEN);
                } catch (ActivityNotFoundException e) {
                    pendingOpen = null;
                    toast("No file picker found");
                    return false;
                }
                return true;
            }
        });
        if (saved != null) {
            web.restoreState(saved);
        } else {
            web.loadUrl(HOME);
        }
        setContentView(web);
    }

    /** A file from the APK's assets/web folder, or a 404. */
    private WebResourceResponse asset(String path) {
        if (path == null || path.isEmpty() || path.endsWith("/")) {
            path = (path == null ? "/" : path) + "index.html";
        }
        if (path.contains("..")) {
            return notFound();
        }
        String ext = path.substring(path.lastIndexOf('.') + 1).toLowerCase();
        String type = TYPES.containsKey(ext) ? TYPES.get(ext) : "application/octet-stream";
        try {
            InputStream in = getAssets().open("web" + path);
            WebResourceResponse r = new WebResourceResponse(type, null, in);
            Map<String, String> headers = new HashMap<>();
            headers.put("Cache-Control", "no-cache");
            r.setResponseHeaders(headers);
            return r;
        } catch (IOException e) {
            return notFound();
        }
    }

    private static WebResourceResponse notFound() {
        return new WebResourceResponse("text/plain", "utf-8", 404, "Not Found", new HashMap<>(), null);
    }

    @Override
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request == REQUEST_OPEN) {
            if (pendingOpen == null) {
                return;
            }
            Uri[] files = null;
            if (result == RESULT_OK && data != null) {
                ClipData clip = data.getClipData();
                if (clip != null) {
                    files = new Uri[clip.getItemCount()];
                    for (int i = 0; i < files.length; i++) {
                        files[i] = clip.getItemAt(i).getUri();
                    }
                } else if (data.getData() != null) {
                    files = new Uri[] { data.getData() };
                }
            }
            pendingOpen.onReceiveValue(files);
            pendingOpen = null;
        } else if (request == REQUEST_SAVE) {
            byte[] bytes = pendingSave;
            pendingSave = null;
            if (result != RESULT_OK || data == null || data.getData() == null || bytes == null) {
                toast("Not saved");
                return;
            }
            try (OutputStream out = getContentResolver().openOutputStream(data.getData(), "wt")) {
                if (out == null) {
                    throw new IOException("no output stream");
                }
                out.write(bytes);
                toast("Saved");
            } catch (IOException e) {
                toast("Couldn't save: " + e.getMessage());
            }
        }
    }

    @Override
    protected void onSaveInstanceState(Bundle out) {
        super.onSaveInstanceState(out);
        web.saveState(out);
    }

    @Override
    public void onBackPressed() {
        if (web.canGoBack()) {
            web.goBack();
        } else {
            super.onBackPressed();
        }
    }

    private void toast(String text) {
        Toast.makeText(this, text, Toast.LENGTH_SHORT).show();
    }

    /** Called from android-shim.js when a web app saves (downloads) a file. */
    private class Bridge {
        @JavascriptInterface
        public void save(final String name, final String type, final String base64) {
            final byte[] bytes;
            try {
                bytes = Base64.decode(base64, Base64.DEFAULT);
            } catch (IllegalArgumentException e) {
                runOnUiThread(() -> toast("Couldn't save " + name));
                return;
            }
            runOnUiThread(() -> {
                // One Save screen at a time: an app can ask twice for one save (GridCraft keeps a
                // Ctrl+S made before the workbook has a name, and saves again once it has one).
                if (pendingSave != null) {
                    return;
                }
                pendingSave = bytes;
                Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.setType(type == null || type.isEmpty() ? "application/octet-stream" : type);
                intent.putExtra(Intent.EXTRA_TITLE, name);
                try {
                    startActivityForResult(intent, REQUEST_SAVE);
                } catch (ActivityNotFoundException e) {
                    pendingSave = null;
                    toast("No app to save files with");
                }
            });
        }
    }
}
