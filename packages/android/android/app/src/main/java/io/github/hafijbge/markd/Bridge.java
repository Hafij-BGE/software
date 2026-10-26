package io.github.hafijbge.markd;

import android.app.Activity;
import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.Context;
import android.content.Intent;
import android.net.Uri;
import android.os.Build;
import android.os.Handler;
import android.os.Looper;
import android.os.Bundle;
import android.os.CancellationSignal;
import android.os.ParcelFileDescriptor;
import android.print.PageRange;
import android.print.PrintAttributes;
import android.print.PrintDocumentAdapter;
import android.print.PrintManager;
import android.provider.DocumentsContract;
import android.util.Base64;
import android.util.Log;
import android.webkit.WebView;
import android.webkit.WebViewClient;

import androidx.annotation.NonNull;
import androidx.webkit.JavaScriptReplyProxy;
import androidx.webkit.WebMessageCompat;
import androidx.webkit.WebViewCompat;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.nio.charset.StandardCharsets;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * Native side of src/native.ts. Messages are `{id, method, args}` JSON; every
 * call is answered once with `{id, ok, result}` or `{id, ok:false, error}`
 * where error is "CODE: message". Only the app's main frame is served.
 */
final class Bridge implements WebViewCompat.WebMessageListener {

    private static final String TAG = "MarkdBridge";

    static final int REQ_OPEN_DOCUMENT = 101;
    static final int REQ_CREATE_DOCUMENT = 102;
    static final int REQ_OPEN_FOLDER = 103;
    static final int REQ_GRANT_FOLDER = 104;
    static final int REQ_PICK_IMAGE = 105;
    static final int REQ_SAVE_FILE = 106;
    static final int REQ_INKWELL = 107;


    private static final int RW = Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION;

    /** One pending JS call. */
    final class Call {
        final String id;
        final JSONObject args;
        private final JavaScriptReplyProxy reply;

        Call(String id, JSONObject args, JavaScriptReplyProxy reply) {
            this.id = id;
            this.args = args;
            this.reply = reply;
        }

        void resolve(Object result) {
            send(true, result, null);
        }

        void reject(String code, String message) {
            send(false, null, code + ": " + message);
        }

        private void send(boolean ok, Object result, String error) {
            ui.post(() -> {
                try {
                    JSONObject json = new JSONObject().put("id", id).put("ok", ok);
                    if (ok) json.put("result", result == null ? JSONObject.NULL : result);
                    else json.put("error", error);
                    reply.postMessage(json.toString());
                } catch (JSONException | IllegalStateException e) {
                    Log.w(TAG, "Cannot reply to " + id, e);
                }
            });
        }
    }

    private final MainActivity activity;
    private final DocStore store;
    private final ExecutorService io = Executors.newSingleThreadExecutor();
    private final Handler ui = new Handler(Looper.getMainLooper());

    /** System picker in flight; only one at a time. */
    private Call pickerCall;
    private int pickerRequest;
    /** Keeps the off-screen print WebView alive while the print UI uses it. */
    private WebView printView;

    Bridge(MainActivity activity, DocStore store) {
        this.activity = activity;
        this.store = store;
    }

    @Override
    public void onPostMessage(@NonNull WebView view, @NonNull WebMessageCompat message, @NonNull Uri sourceOrigin,
                              boolean isMainFrame, @NonNull JavaScriptReplyProxy reply) {
        if (!isMainFrame || message.getData() == null) return;
        try {
            JSONObject json = new JSONObject(message.getData());
            Call call = new Call(json.getString("id"), json.optJSONObject("args") == null ? new JSONObject() : json.getJSONObject("args"), reply);
            dispatch(json.getString("method"), call);
        } catch (JSONException e) {
            Log.w(TAG, "Malformed message", e);
        }
    }

    private void dispatch(String method, Call call) {
        switch (method) {
            case "getInfo": call.resolve(info()); break;
            case "getLaunchDocument": call.resolve(launchDocument()); break;
            case "openDocument": openDocument(call); break;
            case "readDocument": background(call, () -> call.resolve(readDocument(Uri.parse(call.args.getString("uri"))))); break;
            case "writeDocument": background(call, () -> {
                store.writeBytes(Uri.parse(call.args.getString("uri")), call.args.getString("text").getBytes(StandardCharsets.UTF_8));
                call.resolve(true);
            }); break;
            case "createDocument": createDocument(call, REQ_CREATE_DOCUMENT); break;
            case "saveFileAs": createDocument(call, REQ_SAVE_FILE); break;
            case "openFolder": startPicker(call, REQ_OPEN_FOLDER, new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE)); break;
            case "releaseFolder": releaseFolder(call); break;
            case "listFolder": background(call, () -> call.resolve(listFolder(Uri.parse(call.args.getString("treeUri")), call.args.getString("docId")))); break;
            case "createInFolder": background(call, () -> {
                Uri created = store.createInFolder(Uri.parse(call.args.getString("treeUri")), call.args.getString("docId"),
                    "text/markdown", call.args.getString("name"));
                if (created == null) throw new IllegalStateException("The folder refused the new file");
                store.writeBytes(created, new byte[0]);
                call.resolve(new JSONObject().put("uri", created.toString()));
            }); break;
            case "grantFolderAccess": grantFolderAccess(call); break;
            case "saveAsset": background(call, () -> call.resolve(saveAsset(call.args))); break;
            case "pickImage": pickImage(call); break;
            case "inkwellAvailable": call.resolve(true); break;
            case "inkwellSketch": inkwellSketch(call); break;
            case "openHome": activity.openHome(); call.resolve(true); break;
            case "printHtml": printHtml(call); break;
            case "setChrome": activity.setChrome(call.args.optString("top"), call.args.optString("bottom"), call.args.optBoolean("dark")); call.resolve(true); break;
            case "setCurrentDocument": activity.setCurrentDocument(Uri.parse(call.args.optString("uri"))); call.resolve(true); break;
            case "setHandwriting": activity.setHandwriting(call.args.optBoolean("enabled", true)); call.resolve(true); break;
            case "openExternal": openExternal(call); break;
            case "share": share(call); break;
            default: call.reject("UNKNOWN", "Unknown method " + method);
        }
    }

    private interface Task {
        void run() throws Exception;
    }

    private void background(Call call, Task task) {
        io.execute(() -> {
            try {
                task.run();
            } catch (SecurityException e) {
                call.reject("PERMISSION", "Access to this file was revoked. Open it again.");
            } catch (Exception e) {
                Log.w(TAG, "Call failed", e);
                call.reject("IO", e.getMessage() == null ? e.toString() : e.getMessage());
            }
        });
    }

    private JSONObject info() {
        try {
            return new JSONObject()
                .put("sdkInt", Build.VERSION.SDK_INT)
                .put("release", Build.VERSION.RELEASE)
                .put("model", Build.MODEL)
                .put("version", BuildConfig.VERSION_NAME)
                .put("handwritingSupported", Build.VERSION.SDK_INT >= 34);
        } catch (JSONException e) {
            throw new IllegalStateException(e);
        }
    }

    private JSONObject launchDocument() {
        Uri uri = activity.takeLaunchUri();
        try {
            return new JSONObject().put("launchUri", uri == null ? JSONObject.NULL : uri.toString());
        } catch (JSONException e) {
            throw new IllegalStateException(e);
        }
    }

    private JSONObject readDocument(Uri uri) throws Exception {
        String text = store.readText(uri);
        return new JSONObject()
            .put("uri", uri.toString())
            .put("name", store.displayName(uri))
            .put("text", text)
            .put("hasFolder", store.folderOf(uri) != null);
    }

    private JSONArray listFolder(Uri tree, String dirId) throws JSONException {
        JSONArray result = new JSONArray();
        for (DocStore.Entry entry : store.list(tree, dirId)) {
            result.put(new JSONObject()
                .put("name", entry.name)
                .put("docId", entry.docId)
                .put("uri", DocumentsContract.buildDocumentUriUsingTree(tree, entry.docId).toString())
                .put("isDir", entry.isDir())
                .put("modified", entry.modified));
        }
        return result;
    }

    private JSONObject saveAsset(JSONObject args) throws Exception {
        Uri doc = Uri.parse(args.getString("docUri"));
        DocStore.Folder folder = store.folderOf(doc);
        if (folder == null) throw new IllegalStateException("No access to the document's folder");
        byte[] data = Base64.decode(args.getString("base64"), Base64.DEFAULT);
        String path = store.createFile(folder, args.getString("path"), args.optString("mime", "image/png"), data);
        return new JSONObject().put("path", path);
    }

    // ---- System pickers ----

    private void startPicker(Call call, int request, Intent intent) {
        if (pickerCall != null) pickerCall.reject("CANCELLED", "Superseded by another picker");
        pickerCall = call;
        pickerRequest = request;
        try {
            activity.startActivityForResult(intent, request);
        } catch (ActivityNotFoundException e) {
            pickerCall = null;
            call.reject("UNAVAILABLE", "No app can handle this on the device");
        }
    }

    private void openDocument(Call call) {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            // Many file managers label .md files application/octet-stream, so allow any type.
            .setType("*/*")
            .addFlags(RW | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
        startPicker(call, REQ_OPEN_DOCUMENT, intent);
    }

    private void createDocument(Call call, int request) {
        Intent intent = new Intent(Intent.ACTION_CREATE_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType(call.args.optString("mime", "text/markdown"))
            .putExtra(Intent.EXTRA_TITLE, call.args.optString("name", "Untitled.md"));
        startPicker(call, request, intent);
    }

    private void grantFolderAccess(Call call) {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        String uri = call.args.optString("uri", "");
        if (!uri.isEmpty()) intent.putExtra(DocumentsContract.EXTRA_INITIAL_URI, Uri.parse(uri));
        startPicker(call, REQ_GRANT_FOLDER, intent);
    }

    private void pickImage(Call call) {
        Intent intent = new Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("image/*");
        startPicker(call, REQ_PICK_IMAGE, intent);
    }

    private void inkwellSketch(Call call) {
        // The Inkwell canvas is built in (android/inkwell); it answers with a content URI to an SVG.
        Intent intent = new Intent(activity, com.hafij.inkwell.SketchActivity.class)
            .putExtra(com.hafij.inkwell.SketchActivity.EXTRA_PAPER, call.args.optBoolean("paper", true));
        String ink = call.args.optString("ink", "");
        if (!ink.isEmpty()) intent.putExtra(com.hafij.inkwell.SketchActivity.EXTRA_INK, ink);
        startPicker(call, REQ_INKWELL, intent);
    }

    private void releaseFolder(Call call) {
        try {
            activity.getContentResolver().releasePersistableUriPermission(Uri.parse(call.args.getString("treeUri")), RW);
        } catch (Exception ignored) {
            // Already released.
        }
        call.resolve(true);
    }

    /** @return true when the result belonged to a bridge picker. */
    boolean onActivityResult(int requestCode, int resultCode, Intent data) {
        if (pickerCall == null || requestCode != pickerRequest) return false;
        Call call = pickerCall;
        pickerCall = null;
        Uri uri = data == null ? null : data.getData();
        if (resultCode != Activity.RESULT_OK || uri == null) {
            call.reject("CANCELLED", "Cancelled");
            return true;
        }
        int flags = data.getFlags();
        switch (requestCode) {
            case REQ_OPEN_DOCUMENT:
                store.persist(uri, flags);
                background(call, () -> call.resolve(readDocument(uri)));
                break;
            case REQ_CREATE_DOCUMENT:
                store.persist(uri, flags | RW);
                background(call, () -> call.resolve(new JSONObject()
                    .put("uri", uri.toString())
                    .put("name", store.displayName(uri))
                    .put("hasFolder", store.folderOf(uri) != null)));
                break;
            case REQ_SAVE_FILE:
                background(call, () -> {
                    store.writeBytes(uri, Base64.decode(call.args.getString("base64"), Base64.DEFAULT));
                    call.resolve(new JSONObject().put("name", store.displayName(uri)));
                });
                break;
            case REQ_OPEN_FOLDER:
                store.persist(uri, flags | RW);
                background(call, () -> {
                    String rootId = DocumentsContract.getTreeDocumentId(uri);
                    call.resolve(new JSONObject()
                        .put("treeUri", uri.toString())
                        .put("rootId", rootId)
                        .put("name", store.displayName(DocumentsContract.buildDocumentUriUsingTree(uri, rootId))));
                });
                break;
            case REQ_GRANT_FOLDER:
                store.persist(uri, flags | RW);
                background(call, () -> {
                    boolean has = store.folderOf(Uri.parse(call.args.getString("uri"))) != null;
                    if (has) activity.onFolderGranted();
                    call.resolve(new JSONObject().put("hasFolder", has));
                });
                break;
            case REQ_PICK_IMAGE:
                background(call, () -> {
                    String mime = activity.getContentResolver().getType(uri);
                    byte[] bytes = store.readBytes(uri);
                    call.resolve(new JSONObject()
                        .put("mime", mime == null ? "image/png" : mime)
                        .put("base64", Base64.encodeToString(bytes, Base64.NO_WRAP)));
                });
                break;
            case REQ_INKWELL:
                background(call, () -> call.resolve(new JSONObject()
                    .put("svg", new String(store.readBytes(uri), StandardCharsets.UTF_8))));
                break;
            default:
                call.reject("CANCELLED", "Unexpected result");
        }
        return true;
    }

    // ---- Print (PDF export through the system "Save as PDF" printer) ----

    private void printHtml(Call call) {
        String html = call.args.optString("html");
        String job = call.args.optString("jobName", "Markd");
        String size = call.args.optString("pageSize", "A4");
        ui.post(() -> {
            WebView view = new WebView(activity);
            view.getSettings().setJavaScriptEnabled(false);
            view.setWebViewClient(new WebViewClient() {
                private boolean printed;

                @Override
                public void onPageFinished(WebView v, String url) {
                    if (printed) return;
                    printed = true;
                    PrintManager manager = (PrintManager) activity.getSystemService(Context.PRINT_SERVICE);
                    PrintAttributes.MediaSize media = "Letter".equals(size) ? PrintAttributes.MediaSize.NA_LETTER
                        : "Legal".equals(size) ? PrintAttributes.MediaSize.NA_LEGAL
                        : PrintAttributes.MediaSize.ISO_A4;
                    PrintAttributes attributes = new PrintAttributes.Builder().setMediaSize(media).build();
                    manager.print(job, releasing(v.createPrintDocumentAdapter(job)), attributes);
                    call.resolve(true);
                }
            });
            printView = view;
            view.loadDataWithBaseURL(MainActivity.ORIGIN + "/print/", html, "text/html", "UTF-8", null);
        });
    }

    /** Delegates to the WebView adapter and drops the off-screen WebView once printing ends. */
    private PrintDocumentAdapter releasing(PrintDocumentAdapter inner) {
        return new PrintDocumentAdapter() {
            @Override
            public void onStart() {
                inner.onStart();
            }

            @Override
            public void onLayout(PrintAttributes oldAttributes, PrintAttributes newAttributes, CancellationSignal cancel,
                                 LayoutResultCallback callback, Bundle extras) {
                inner.onLayout(oldAttributes, newAttributes, cancel, callback, extras);
            }

            @Override
            public void onWrite(PageRange[] pages, ParcelFileDescriptor destination, CancellationSignal cancel,
                                WriteResultCallback callback) {
                inner.onWrite(pages, destination, cancel, callback);
            }

            @Override
            public void onFinish() {
                inner.onFinish();
                if (printView != null) {
                    printView.destroy();
                    printView = null;
                }
            }
        };
    }

    // ---- Intents ----

    private void openExternal(Call call) {
        String url = call.args.optString("url");
        String scheme = Uri.parse(url).getScheme();
        if (!"http".equals(scheme) && !"https".equals(scheme) && !"mailto".equals(scheme)) {
            call.reject("BLOCKED", "Only web and mail links can be opened");
            return;
        }
        try {
            activity.startActivity(new Intent(Intent.ACTION_VIEW, Uri.parse(url)));
            call.resolve(true);
        } catch (ActivityNotFoundException e) {
            call.reject("UNAVAILABLE", "No app can open this link");
        }
    }

    private void share(Call call) {
        Uri uri = Uri.parse(call.args.optString("uri"));
        Intent send = new Intent(Intent.ACTION_SEND)
            .setType(call.args.optString("mime", "text/markdown"))
            .putExtra(Intent.EXTRA_STREAM, uri)
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        send.setClipData(ClipData.newRawUri("", uri));
        activity.startActivity(Intent.createChooser(send, null));
        call.resolve(true);
    }
}
