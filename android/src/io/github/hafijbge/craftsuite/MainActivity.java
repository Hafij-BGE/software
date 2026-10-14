package io.github.hafijbge.craftsuite;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.Intent;
import android.content.SharedPreferences;
import android.database.Cursor;
import android.net.Uri;
import android.provider.OpenableColumns;
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

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * Craft Suite for Android: the launcher page and WordCraft, DeckCraft and GridCraft's web versions
 * (WebAssembly), shipped in the APK's assets and shown in a WebView.
 *
 * The pages load from https://appassets.androidplatform.net/, served from the assets here (a
 * secure origin, with the right content types for .wasm). Opening a file uses the system's file
 * picker; saving (the web apps' downloads, passed on by android-shim.js) asks where to save
 * through the system's Create document screen.
 *
 * Files opened with Craft Suite from another app (Open with, Share) or from the launcher's Open
 * button go to the app for their type, which receives them as a dropped file (android-shim.js).
 * The launcher lists recently opened files it may open again.
 *
 * Read-only: "Craft Suite Viewer" (an alias of this activity in Open with) and the launcher's
 * Read-only switch open the file with ?readonly=1, which android-shim.js turns into each app's
 * reading mode; its Edit button opens the same file again, editable.
 */
public class MainActivity extends Activity {
    private static final String HOST = "appassets.androidplatform.net";
    private static final String HOME = "https://" + HOST + "/index.html";
    private static final int REQUEST_OPEN = 1;
    private static final int REQUEST_SAVE = 2;
    private static final int REQUEST_PICK = 3;
    /** The largest file opened (it travels to the page in memory). */
    private static final long MAX_OPEN = 100L * 1024 * 1024;
    private static final int MAX_RECENT = 12;

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
    /** A file waiting for its app's page to take it: {name, type, data (base64)}. */
    private String pendingFile;
    /** The last file a page took, for Edit in read-only mode (it opens again, editable). */
    private String lastFile;
    /** The launcher's Read-only switch, for the file being picked. */
    private boolean pickReadOnly;

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
        setContentView(web);
        if (!openIntent(getIntent())) {
            if (saved != null) {
                web.restoreState(saved);
            } else {
                web.loadUrl(HOME);
            }
        }
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        openIntent(intent);
    }

    /** Open with / Share: true when the intent carried a file (now opening). */
    private boolean openIntent(Intent intent) {
        if (intent == null) {
            return false;
        }
        Uri uri = null;
        if (Intent.ACTION_VIEW.equals(intent.getAction())) {
            uri = intent.getData();
        } else if (Intent.ACTION_SEND.equals(intent.getAction())) {
            uri = intent.getParcelableExtra(Intent.EXTRA_STREAM);
        }
        if (uri == null) {
            return false;
        }
        boolean viewer = intent.getComponent() != null && intent.getComponent().getClassName().endsWith(".Viewer");
        // Handled once (not again when the activity is recreated).
        intent.setAction(Intent.ACTION_MAIN);
        openUri(uri, intent.getType(), viewer);
        return true;
    }

    /** The app a file belongs to, from its name (then its type): word, deck, grid or null. */
    static String appFor(String name, String type) {
        String n = name == null ? "" : name.toLowerCase(Locale.ROOT);
        String ext = n.contains(".") ? n.substring(n.lastIndexOf('.') + 1) : "";
        switch (ext) {
            case "docx": case "docm": case "dotx": case "dotm": case "doc": case "dot": case "odt": case "rtf":
            case "md": case "markdown": case "tex": case "html": case "htm": case "txt":
                return "word";
            case "pptx": case "potx": case "ppsx": case "pptm": case "deckcraft":
                return "deck";
            case "xlsx": case "xlsm": case "xlsb": case "ods": case "csv": case "tsv":
                return "grid";
            default:
                break;
        }
        String t = type == null ? "" : type.toLowerCase(Locale.ROOT);
        if (t.contains("wordprocessing") || t.contains("msword") || t.contains("opendocument.text") || t.contains("rtf")
                || t.equals("text/markdown") || t.equals("text/plain") || t.equals("text/html")) {
            return "word";
        }
        if (t.contains("presentation") || t.contains("powerpoint")) {
            return "deck";
        }
        if (t.contains("spreadsheet") || t.contains("excel") || t.equals("text/csv") || t.equals("text/comma-separated-values")
                || t.equals("text/tab-separated-values")) {
            return "grid";
        }
        return null;
    }

    private String displayName(Uri uri) {
        String name = null;
        try (Cursor c = getContentResolver().query(uri, new String[] { OpenableColumns.DISPLAY_NAME }, null, null, null)) {
            if (c != null && c.moveToFirst()) {
                name = c.getString(0);
            }
        } catch (RuntimeException e) {
            // Not a content provider that knows names (a file:// link).
        }
        if (name == null || name.isEmpty()) {
            name = uri.getLastPathSegment();
        }
        return name == null || name.isEmpty() ? "document" : name;
    }

    /** Reads the file and opens its app's page, which takes it (takeFile). */
    private void openUri(Uri uri, String type, boolean readOnly) {
        String name = displayName(uri);
        if (type == null || type.isEmpty()) {
            type = getContentResolver().getType(uri);
        }
        String app = appFor(name, type);
        if (app == null) {
            toast("Craft Suite can't open " + name);
            web.loadUrl(HOME);
            return;
        }
        try (InputStream in = getContentResolver().openInputStream(uri)) {
            if (in == null) {
                throw new IOException("no input stream");
            }
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            byte[] buf = new byte[64 * 1024];
            long total = 0;
            int n;
            while ((n = in.read(buf)) > 0) {
                total += n;
                if (total > MAX_OPEN) {
                    throw new IOException("larger than 100 MB");
                }
                out.write(buf, 0, n);
            }
            JSONObject f = new JSONObject();
            f.put("name", name);
            f.put("type", type == null ? "" : type);
            f.put("data", Base64.encodeToString(out.toByteArray(), Base64.NO_WRAP));
            pendingFile = f.toString();
        } catch (IOException | SecurityException | JSONException e) {
            toast("Couldn't open " + name + ": " + e.getMessage());
            web.loadUrl(HOME);
            return;
        }
        remember(uri, name, app);
        web.loadUrl("https://" + HOST + "/" + app + "/index.html" + (readOnly ? "?readonly=1" : ""));
    }

    // ---- recent files (the launcher's list) ----------------------------------------------------

    private SharedPreferences prefs() {
        return getSharedPreferences("craft-suite", MODE_PRIVATE);
    }

    private JSONArray recentList() {
        try {
            return new JSONArray(prefs().getString("recent", "[]"));
        } catch (JSONException e) {
            return new JSONArray();
        }
    }

    /** Remembers a file it can open again (when Android lets it keep access). */
    private void remember(Uri uri, String name, String app) {
        boolean kept = false;
        try {
            getContentResolver().takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION);
            kept = true;
        } catch (SecurityException e) {
            // A one-time permission (most Open with links): not listed.
        }
        if (!kept) {
            return;
        }
        JSONArray old = recentList();
        JSONArray list = new JSONArray();
        try {
            JSONObject item = new JSONObject();
            item.put("uri", uri.toString());
            item.put("name", name);
            item.put("app", app);
            item.put("time", System.currentTimeMillis());
            list.put(item);
            for (int i = 0; i < old.length() && list.length() < MAX_RECENT; i++) {
                JSONObject o = old.optJSONObject(i);
                if (o != null && !uri.toString().equals(o.optString("uri"))) {
                    list.put(o);
                }
            }
        } catch (JSONException e) {
            return;
        }
        prefs().edit().putString("recent", list.toString()).apply();
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
        } else if (request == REQUEST_PICK) {
            if (result == RESULT_OK && data != null && data.getData() != null) {
                openUri(data.getData(), null, pickReadOnly);
            }
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

    /** Called from the pages: android-shim.js (saving, opening) and the launcher. */
    private class Bridge {
        @JavascriptInterface
        public boolean hasFile() {
            return pendingFile != null;
        }

        /** The file waiting to open, once ({name, type, data}); null when none. */
        @JavascriptInterface
        public String takeFile() {
            String f = pendingFile;
            pendingFile = null;
            if (f != null) {
                lastFile = f;
            }
            return f;
        }

        /** Read-only mode's Edit: the last file waits again, for the page to reload editable. */
        @JavascriptInterface
        public boolean reopen() {
            if (lastFile == null) {
                return false;
            }
            pendingFile = lastFile;
            return true;
        }

        @JavascriptInterface
        public void toast(final String text) {
            runOnUiThread(() -> MainActivity.this.toast(text));
        }

        /** The launcher's Open button: the system's file picker. */
        @JavascriptInterface
        public void openFile(final boolean readOnly) {
            runOnUiThread(() -> {
                pickReadOnly = readOnly;
                Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                intent.addCategory(Intent.CATEGORY_OPENABLE);
                intent.setType("*/*");
                try {
                    startActivityForResult(intent, REQUEST_PICK);
                } catch (ActivityNotFoundException e) {
                    toast("No file picker found");
                }
            });
        }

        @JavascriptInterface
        public String recent() {
            return recentList().toString();
        }

        @JavascriptInterface
        public void openRecent(final String uri, final boolean readOnly) {
            runOnUiThread(() -> openUri(Uri.parse(uri), null, readOnly));
        }

        @JavascriptInterface
        public void forgetRecent(final String uri) {
            JSONArray old = recentList();
            JSONArray list = new JSONArray();
            for (int i = 0; i < old.length(); i++) {
                JSONObject o = old.optJSONObject(i);
                if (o != null && !uri.equals(o.optString("uri"))) {
                    list.put(o);
                }
            }
            prefs().edit().putString("recent", list.toString()).apply();
            try {
                getContentResolver().releasePersistableUriPermission(Uri.parse(uri), Intent.FLAG_GRANT_READ_URI_PERMISSION);
            } catch (SecurityException e) {
                // Already gone.
            }
        }

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
