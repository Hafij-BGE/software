package com.hafij.inkwell;

import android.app.Activity;
import android.app.AlertDialog;
import android.app.ProgressDialog;
import android.content.ContentResolver;
import android.content.Intent;
import android.content.SharedPreferences;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.graphics.pdf.PdfRenderer;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract;
import android.text.Editable;
import android.text.TextWatcher;
import android.text.format.DateUtils;
import android.util.LruCache;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.inputmethod.InputMethodManager;
import android.widget.BaseAdapter;
import android.widget.Button;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.GridView;
import android.widget.HorizontalScrollView;
import android.widget.ImageButton;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.PopupMenu;
import android.widget.TextView;
import android.widget.Toast;

import com.hafij.inkwell.io.InkFile;
import com.hafij.inkwell.model.Doc;
import com.hafij.inkwell.pdf.PdfExport;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.Reader;
import java.nio.charset.StandardCharsets;
import java.text.SimpleDateFormat;
import java.util.ArrayDeque;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Date;
import java.util.Deque;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Set;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/**
 * Markd's home: every Markdown file, drawing and annotated PDF in the library folder the
 * user picked, plus recently opened files. Markdown opens in Markd's editor; drawings and
 * PDFs open in the Inkwell canvas.
 */
public class HomeActivity extends Activity {
    private static final int REQ_TREE = 1, REQ_PDF = 2, REQ_IMPORT = 3;
    private static final String KEY_TREE = "libraryTree";
    private static final String EDITOR_CLASS = "io.github.hafijbge.markd.MainActivity";

    enum Kind {DIR, MARKDOWN, NOTE, PDF}

    private final ExecutorService io = Executors.newFixedThreadPool(3);
    private final Handler main = new Handler(Looper.getMainLooper());
    private final LruCache<String, Bitmap> thumbs = new LruCache<String, Bitmap>(32 * 1024 * 1024) {
        @Override
        protected int sizeOf(String k, Bitmap v) {
            return v.getByteCount();
        }
    };
    private final Set<String> loading = new HashSet<>();
    private SharedPreferences prefs;
    private Uri tree;
    /** Folder stack: document ids and names from the library root down. */
    private final Deque<String[]> path = new ArrayDeque<>();
    private final List<SafDocs.Entry> items = new ArrayList<>();
    private Adapter adapter;
    private GridView grid;
    private TextView titleView, emptyView;
    private ImageButton upBtn;
    private EditText search;
    private LinearLayout recentRow;
    private View recentBox, setupBox;
    private String query = "";
    private int listGeneration;
    private Uri pendingImport;
    /** Note uri|modified → {pdf uri or null, pdf file name}; empty array for drawings. */
    private final java.util.Map<String, String[]> linkCache = new java.util.concurrent.ConcurrentHashMap<>();
    /** PDF document id or "name:" + file name → notes in the shown folder that annotate it. */
    private final java.util.Map<String, List<SafDocs.Entry>> links = new java.util.HashMap<>();

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        Inkwell.init(this);
        prefs = getSharedPreferences("inkwell", MODE_PRIVATE);
        String t = prefs.getString(KEY_TREE, null);
        tree = t == null ? null : Uri.parse(t);
        if (tree != null && !hasGrant(tree)) tree = null;
        buildUi();
        handleIntent(getIntent());
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        handleIntent(intent);
    }

    @Override
    protected void onResume() {
        super.onResume();
        refresh();
    }

    private boolean hasGrant(Uri t) {
        for (android.content.UriPermission p : getContentResolver().getPersistedUriPermissions()) {
            if (p.getUri().equals(t) && p.isReadPermission() && p.isWritePermission()) return true;
        }
        return false;
    }

    private String currentId() {
        return path.isEmpty() ? SafDocs.rootId(tree) : path.peekLast()[0];
    }

    // ------------------------------------------------------------------ UI

    private void buildUi() {
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setBackgroundColor(Ui.surface(this));
        root.setFitsSystemWindows(true);

        LinearLayout top = new LinearLayout(this);
        top.setGravity(Gravity.CENTER_VERTICAL);
        top.setPadding(Ui.dp(this, 4), Ui.dp(this, 4), Ui.dp(this, 4), Ui.dp(this, 4));
        upBtn = Ui.icon(this, R.drawable.ic_back, "Up", v -> goUp());
        top.addView(upBtn);
        titleView = Ui.label(this, "Markd", 22);
        titleView.setPadding(Ui.dp(this, 8), 0, 0, 0);
        top.addView(titleView, new LinearLayout.LayoutParams(0, Ui.dp(this, 48), 1));
        top.addView(Ui.icon(this, R.drawable.ic_search, "Search", v -> toggleSearch()));
        ImageButton sort = Ui.icon(this, R.drawable.ic_sort, "Sort", null);
        sort.setOnClickListener(v -> sortMenu(sort));
        top.addView(sort);
        ImageButton more = Ui.icon(this, R.drawable.ic_more, "More", null);
        more.setOnClickListener(v -> moreMenu(more));
        top.addView(more);
        root.addView(top);

        search = new EditText(this);
        search.setHint("Search files");
        search.setSingleLine(true);
        search.setVisibility(View.GONE);
        LinearLayout.LayoutParams slp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        slp.leftMargin = slp.rightMargin = Ui.dp(this, 16);
        search.addTextChangedListener(new TextWatcher() {
            @Override
            public void beforeTextChanged(CharSequence s, int a, int b, int c) {
            }

            @Override
            public void onTextChanged(CharSequence s, int a, int b, int c) {
            }

            @Override
            public void afterTextChanged(Editable e) {
                query = e.toString().trim().toLowerCase(Locale.getDefault());
                refresh();
            }
        });
        root.addView(search, slp);

        // Recent row
        LinearLayout recent = new LinearLayout(this);
        recent.setOrientation(LinearLayout.VERTICAL);
        TextView rl = Ui.label(this, "Recent", 14);
        rl.setAlpha(0.7f);
        rl.setPadding(Ui.dp(this, 16), Ui.dp(this, 4), 0, Ui.dp(this, 4));
        recent.addView(rl);
        HorizontalScrollView hs = new HorizontalScrollView(this);
        hs.setHorizontalScrollBarEnabled(false);
        recentRow = new LinearLayout(this);
        recentRow.setPadding(Ui.dp(this, 12), 0, Ui.dp(this, 12), Ui.dp(this, 4));
        hs.addView(recentRow);
        recent.addView(hs);
        TextView fl = Ui.label(this, "Files", 14);
        fl.setAlpha(0.7f);
        fl.setPadding(Ui.dp(this, 16), Ui.dp(this, 8), 0, 0);
        recent.addView(fl);
        recentBox = recent;
        root.addView(recent);

        FrameLayout frame = new FrameLayout(this);
        grid = new GridView(this);
        grid.setColumnWidth(Ui.dp(this, 150));
        grid.setNumColumns(GridView.AUTO_FIT);
        grid.setStretchMode(GridView.STRETCH_COLUMN_WIDTH);
        grid.setVerticalSpacing(Ui.dp(this, 12));
        grid.setHorizontalSpacing(Ui.dp(this, 12));
        int p = Ui.dp(this, 12);
        grid.setPadding(p, p, p, Ui.dp(this, 96));
        grid.setClipToPadding(false);
        grid.setSelector(new android.graphics.drawable.ColorDrawable(Color.TRANSPARENT));
        adapter = new Adapter();
        grid.setAdapter(adapter);
        grid.setOnItemClickListener((parent, view, pos, id) -> open(items.get(pos)));
        grid.setOnItemLongClickListener((parent, view, pos, id) -> {
            itemMenu(view, items.get(pos));
            return true;
        });
        frame.addView(grid);

        emptyView = Ui.label(this, "", 16);
        emptyView.setSingleLine(false);
        emptyView.setGravity(Gravity.CENTER);
        emptyView.setAlpha(0.6f);
        frame.addView(emptyView, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));

        setupBox = buildSetup();
        frame.addView(setupBox, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));

        ImageButton fab = new ImageButton(this);
        fab.setImageResource(R.drawable.ic_plus);
        fab.setColorFilter(Color.WHITE);
        GradientDrawable bg = new GradientDrawable();
        bg.setShape(GradientDrawable.OVAL);
        bg.setColor(Ui.accent(this));
        fab.setBackground(bg);
        fab.setElevation(Ui.dp(this, 6));
        fab.setContentDescription("New");
        int fp = Ui.dp(this, 16);
        fab.setPadding(fp, fp, fp, fp);
        fab.setScaleType(ImageView.ScaleType.FIT_CENTER);
        fab.setOnClickListener(v -> newMenu(fab));
        FrameLayout.LayoutParams flp = new FrameLayout.LayoutParams(Ui.dp(this, 60), Ui.dp(this, 60), Gravity.BOTTOM | Gravity.END);
        flp.setMargins(0, 0, Ui.dp(this, 20), Ui.dp(this, 20));
        frame.addView(fab, flp);
        root.addView(frame, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1));
        setContentView(root);
    }

    private View buildSetup() {
        LinearLayout box = new LinearLayout(this);
        box.setOrientation(LinearLayout.VERTICAL);
        box.setGravity(Gravity.CENTER);
        int p = Ui.dp(this, 32);
        box.setPadding(p, p, p, p);
        box.setBackgroundColor(Ui.surface(this));
        TextView h = Ui.label(this, "Choose a folder for your files", 20);
        h.setGravity(Gravity.CENTER);
        box.addView(h);
        TextView t = Ui.label(this, "Markdown files, drawings and annotated PDFs are kept there. "
                + "Other apps and your computer can see them, and they stay when the app is removed. "
                + "A folder such as Documents/Markd works well.", 15);
        t.setSingleLine(false);
        t.setGravity(Gravity.CENTER);
        t.setAlpha(0.75f);
        t.setPadding(0, Ui.dp(this, 12), 0, Ui.dp(this, 20));
        box.addView(t);
        Button b = new Button(this);
        b.setText("Choose folder");
        b.setOnClickListener(v -> chooseFolder());
        box.addView(b);
        return box;
    }

    private void toggleSearch() {
        boolean show = search.getVisibility() != View.VISIBLE;
        search.setVisibility(show ? View.VISIBLE : View.GONE);
        InputMethodManager im = (InputMethodManager) getSystemService(INPUT_METHOD_SERVICE);
        if (show) {
            search.requestFocus();
            im.showSoftInput(search, 0);
        } else {
            search.setText("");
            im.hideSoftInputFromWindow(search.getWindowToken(), 0);
        }
    }

    private void refresh() {
        boolean ready = tree != null;
        setupBox.setVisibility(ready ? View.GONE : View.VISIBLE);
        buildRecents();
        if (!ready) {
            items.clear();
            adapter.notifyDataSetChanged();
            upBtn.setVisibility(View.GONE);
            titleView.setText("Markd");
            emptyView.setVisibility(View.GONE);
            return;
        }
        boolean searching = !query.isEmpty();
        upBtn.setVisibility(path.isEmpty() || searching ? View.GONE : View.VISIBLE);
        titleView.setText(searching ? "Search" : path.isEmpty() ? "Markd" : path.peekLast()[1]);
        int gen = ++listGeneration;
        String parent = currentId();
        String q = query;
        io.execute(() -> {
            List<SafDocs.Entry> found = new ArrayList<>();
            if (q.isEmpty()) {
                for (SafDocs.Entry e : SafDocs.list(getContentResolver(), tree, parent)) if (kind(e) != null) found.add(e);
            } else {
                collect(parent, q, found, 0);
            }
            java.util.Map<String, List<SafDocs.Entry>> l = linksIn(found);
            main.post(() -> {
                if (gen != listGeneration) return;
                links.clear();
                links.putAll(l);
                showItems(found, searching);
            });
        });
    }

    private void collect(String parentId, String q, List<SafDocs.Entry> out, int depth) {
        if (depth > 6 || out.size() > 300) return;
        for (SafDocs.Entry e : SafDocs.list(getContentResolver(), tree, parentId)) {
            Kind k = kind(e);
            if (k == null) continue;
            if (e.name.toLowerCase(Locale.getDefault()).contains(q)) out.add(e);
            if (k == Kind.DIR) collect(e.docId, q, out, depth + 1);
        }
    }

    private void showItems(List<SafDocs.Entry> found, boolean searching) {
        Set<String> pinned = prefs.getStringSet("pinnedDocs", Collections.emptySet());
        boolean byName = "name".equals(prefs.getString("sort", "modified"));
        Collections.sort(found, (a, b) -> {
            if (a.isDir() != b.isDir()) return a.isDir() ? -1 : 1;
            boolean pa = pinned.contains(a.uri.toString()), pb = pinned.contains(b.uri.toString());
            if (pa != pb) return pa ? -1 : 1;
            if (byName) return a.name.compareToIgnoreCase(b.name);
            return Long.compare(b.modified, a.modified);
        });
        items.clear();
        items.addAll(found);
        adapter.notifyDataSetChanged();
        emptyView.setVisibility(items.isEmpty() ? View.VISIBLE : View.GONE);
        emptyView.setText(searching ? "Nothing matches “" + query + "”."
                : "Nothing here yet.\nTap + to write Markdown, start a drawing or annotate a PDF.");
    }

    static Kind kind(SafDocs.Entry e) {
        String n = e.name.toLowerCase(Locale.ROOT);
        if (n.startsWith(".")) return null;
        // Markdown images (sketches, pasted pictures) live in assets/; keep them out of the grid.
        if (e.isDir()) return "assets".equals(n) ? null : Kind.DIR;
        if (n.endsWith(".md") || n.endsWith(".markdown") || n.endsWith(".mdown") || n.endsWith(".txt")) return Kind.MARKDOWN;
        if (n.endsWith(InkFile.EXT) || n.endsWith(".xopp")) return Kind.NOTE;
        if (n.endsWith(InkFile.BG_SUFFIX)) return null;
        if (n.endsWith(".pdf")) return Kind.PDF;
        return null;
    }

    private static Kind kindOfName(String name) {
        String n = name.toLowerCase(Locale.ROOT);
        if (n.endsWith(".md") || n.endsWith(".markdown") || n.endsWith(".mdown") || n.endsWith(".txt")) return Kind.MARKDOWN;
        if (n.endsWith(InkFile.EXT) || n.endsWith(".xopp")) return Kind.NOTE;
        if (n.endsWith(".pdf")) return Kind.PDF;
        return null;
    }

    private void goUp() {
        if (!path.isEmpty()) {
            path.removeLast();
            refresh();
        }
    }

    @Override
    public void onBackPressed() {
        if (search.getVisibility() == View.VISIBLE) toggleSearch();
        else if (!path.isEmpty()) goUp();
        else super.onBackPressed();
    }

    // ------------------------------------------------------------------ recent row

    private void buildRecents() {
        recentRow.removeAllViews();
        List<Recents.Item> list = Recents.list(this);
        boolean show = !list.isEmpty() && path.isEmpty() && query.isEmpty();
        recentBox.setVisibility(show ? View.VISIBLE : View.GONE);
        if (!show) return;
        for (Recents.Item it : list) {
            Kind k = kindOfName(it.name);
            if (k == null) continue;
            LinearLayout chip = new LinearLayout(this);
            chip.setGravity(Gravity.CENTER_VERTICAL);
            chip.setPadding(Ui.dp(this, 10), Ui.dp(this, 8), Ui.dp(this, 14), Ui.dp(this, 8));
            GradientDrawable bg = new GradientDrawable();
            bg.setCornerRadius(Ui.dp(this, 14));
            bg.setColor(Ui.withAlpha(Ui.text(this), 0x10));
            chip.setBackground(bg);
            ImageView icon = new ImageView(this);
            icon.setImageResource(iconFor(k));
            chip.addView(icon, new LinearLayout.LayoutParams(Ui.dp(this, 22), Ui.dp(this, 22)));
            LinearLayout texts = new LinearLayout(this);
            texts.setOrientation(LinearLayout.VERTICAL);
            texts.setPadding(Ui.dp(this, 8), 0, 0, 0);
            TextView name = Ui.label(this, displayTitle(it.name), 14);
            name.setMaxWidth(Ui.dp(this, 200));
            texts.addView(name);
            TextView when = Ui.label(this, DateUtils.getRelativeTimeSpanString(it.time).toString(), 12);
            when.setAlpha(0.6f);
            texts.addView(when);
            chip.addView(texts);
            LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
            lp.rightMargin = Ui.dp(this, 8);
            chip.setOnClickListener(v -> openRecent(it, k));
            chip.setOnLongClickListener(v -> {
                PopupMenu m = new PopupMenu(this, v);
                m.getMenu().add("Remove from Recent");
                m.setOnMenuItemClickListener(x -> {
                    Recents.remove(this, it.uri);
                    buildRecents();
                    return true;
                });
                m.show();
                return true;
            });
            recentRow.addView(chip, lp);
        }
    }

    private static int iconFor(Kind k) {
        switch (k) {
            case DIR: return R.drawable.ic_folder;
            case MARKDOWN: return R.drawable.ic_tool_text;
            case PDF: return R.drawable.ic_file_pdf;
            default: return R.drawable.ic_tool_pencil;
        }
    }

    private static String displayTitle(String name) {
        Kind k = kindOfName(name);
        if (k == Kind.NOTE || k == Kind.MARKDOWN) {
            int dot = name.lastIndexOf('.');
            return dot > 0 ? name.substring(0, dot) : name;
        }
        return name;
    }

    private void openRecent(Recents.Item it, Kind k) {
        Uri uri = Uri.parse(it.uri);
        if (k == Kind.MARKDOWN) {
            openMarkdown(uri, it.name);
            return;
        }
        // Notes must live in the library tree; find their folder for the attached PDF.
        if (tree == null) return;
        try {
            DocumentsContract.Path p = DocumentsContract.findDocumentPath(getContentResolver(), uri);
            List<String> ids = p == null ? null : p.getPath();
            if (ids == null || ids.size() < 2) throw new IOException("not found");
            String parent = ids.get(ids.size() - 2);
            if (k == Kind.PDF) {
                annotate(new SafDocs.Entry(uri, ids.get(ids.size() - 1), it.name, "application/pdf", 0, 0), parent);
            } else {
                openNote(uri, parent, it.name);
            }
        } catch (IOException | RuntimeException e) {
            Toast.makeText(this, it.name + " is no longer available", Toast.LENGTH_SHORT).show();
            Recents.remove(this, it.uri);
            buildRecents();
        }
    }

    // ------------------------------------------------------------------ PDF ↔ note links

    /** The PDF a note annotates (cached), or null for a drawing. Runs off the UI thread. */
    private String[] linkOf(SafDocs.Entry note) {
        String key = note.uri + "|" + note.modified;
        String[] l = linkCache.get(key);
        if (l == null) {
            try {
                l = InkFile.readPdfLink(getContentResolver().openInputStream(note.uri));
            } catch (IOException | RuntimeException e) {
                l = null;
            }
            if (l == null) l = new String[0];
            linkCache.put(key, l);
        }
        return l.length == 0 ? null : l;
    }

    private static String pdfKey(String uri, String name) {
        if (uri != null) {
            try {
                return DocumentsContract.getDocumentId(Uri.parse(uri));
            } catch (RuntimeException ignored) {
                // not a document uri; fall back to the name
            }
        }
        return "name:" + name;
    }

    /** Groups the notes in {@code entries} by the PDF they annotate. */
    private java.util.Map<String, List<SafDocs.Entry>> linksIn(List<SafDocs.Entry> entries) {
        java.util.Map<String, List<SafDocs.Entry>> map = new java.util.HashMap<>();
        for (SafDocs.Entry e : entries) {
            if (kind(e) != Kind.NOTE) continue;
            String[] l = linkOf(e);
            if (l == null) continue;
            // A note may be found by its PDF's document id or, for desktop notes, by file name.
            for (String key : new String[]{l[0] == null ? null : pdfKey(l[0], l[1]), "name:" + l[1]}) {
                if (key == null) continue;
                List<SafDocs.Entry> list = map.get(key);
                if (list == null) map.put(key, list = new ArrayList<>());
                if (!list.contains(e)) list.add(e);
            }
        }
        return map;
    }

    /** Notes in the PDF's folder that annotate it, newest first. Runs off the UI thread. */
    private List<SafDocs.Entry> notesFor(SafDocs.Entry pdf, String parentId) {
        java.util.Map<String, List<SafDocs.Entry>> map = linksIn(SafDocs.list(getContentResolver(), tree, parentId));
        List<SafDocs.Entry> out = new ArrayList<>();
        List<SafDocs.Entry> byId = map.get(pdf.docId);
        if (byId != null) out.addAll(byId);
        List<SafDocs.Entry> byName = map.get("name:" + pdf.name);
        if (byName != null) {
            for (SafDocs.Entry e : byName) {
                String[] l = linkOf(e);
                // Name matches only count when the note has no uri, or its uri is this PDF.
                if (!out.contains(e) && (l[0] == null || pdfKey(l[0], l[1]).equals(pdf.docId))) out.add(e);
            }
        }
        out.sort((a, b) -> Long.compare(b.modified, a.modified));
        return out;
    }

    private int linkedCount(SafDocs.Entry pdf) {
        Set<SafDocs.Entry> all = new HashSet<>();
        List<SafDocs.Entry> a = links.get(pdf.docId);
        if (a != null) all.addAll(a);
        List<SafDocs.Entry> b = links.get("name:" + pdf.name);
        if (b != null) all.addAll(b);
        return all.size();
    }

    // ------------------------------------------------------------------ opening

    private void open(SafDocs.Entry e) {
        Kind k = kind(e);
        if (k == null) return;
        switch (k) {
            case DIR:
                if (search.getVisibility() == View.VISIBLE) toggleSearch();
                path.addLast(new String[]{e.docId, e.name});
                refresh();
                break;
            case MARKDOWN:
                openMarkdown(e.uri, e.name);
                break;
            case NOTE:
                openNote(e.uri, parentOf(e), e.name);
                break;
            case PDF:
                annotate(e, parentOf(e));
                break;
            default:
                break;
        }
    }

    /** Parent id of an entry shown in the grid (search results may sit in subfolders). */
    private String parentOf(SafDocs.Entry e) {
        if (query.isEmpty()) return currentId();
        try {
            DocumentsContract.Path p = DocumentsContract.findDocumentPath(getContentResolver(), e.uri);
            List<String> ids = p == null ? null : p.getPath();
            if (ids != null && ids.size() >= 2) return ids.get(ids.size() - 2);
        } catch (IOException | RuntimeException ignored) {
            // fall back
        }
        return currentId();
    }

    private void openMarkdown(Uri uri, String name) {
        Recents.add(this, uri.toString(), name);
        Intent i = new Intent(Intent.ACTION_VIEW);
        i.setDataAndType(uri, "text/markdown");
        i.setClassName(getPackageName(), EDITOR_CLASS);
        i.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
        startActivity(i);
    }

    private void openNote(Uri uri, String parentId, String name) {
        Recents.add(this, uri.toString(), name);
        Intent i = new Intent(this, EditorActivity.class);
        i.putExtra(EditorActivity.EXTRA_DOC_URI, uri.toString());
        i.putExtra(EditorActivity.EXTRA_TREE_URI, tree.toString());
        i.putExtra(EditorActivity.EXTRA_PARENT_ID, parentId);
        i.putExtra(EditorActivity.EXTRA_NAME, name.toLowerCase(Locale.ROOT).endsWith(".xopp")
                ? name.substring(0, name.length() - 5) + InkFile.EXT : name);
        startActivity(i);
    }

    /**
     * Tapping a PDF: no notes yet → start one; one → open it; several → choose one, delete
     * one, or start another.
     */
    private void annotate(SafDocs.Entry pdf, String parentId) {
        io.execute(() -> {
            List<SafDocs.Entry> notes = notesFor(pdf, parentId);
            main.post(() -> {
                if (notes.isEmpty()) newAnnotation(pdf.uri, pdf.name, parentId);
                else if (notes.size() == 1) openNote(notes.get(0).uri, parentId, notes.get(0).name);
                else chooseAnnotation(pdf, parentId, notes);
            });
        });
    }

    private void chooseAnnotation(SafDocs.Entry pdf, String parentId, List<SafDocs.Entry> notes) {
        LinearLayout list = new LinearLayout(this);
        list.setOrientation(LinearLayout.VERTICAL);
        list.setPadding(Ui.dp(this, 12), Ui.dp(this, 4), Ui.dp(this, 12), 0);
        AlertDialog[] dialog = new AlertDialog[1];
        for (SafDocs.Entry n : notes) {
            LinearLayout row = new LinearLayout(this);
            row.setGravity(Gravity.CENTER_VERTICAL);
            row.setBackground(Ui.ripple(this, 0));
            row.setPadding(Ui.dp(this, 8), Ui.dp(this, 6), 0, Ui.dp(this, 6));
            ImageView icon = new ImageView(this);
            icon.setImageResource(R.drawable.ic_tool_pencil);
            row.addView(icon, new LinearLayout.LayoutParams(Ui.dp(this, 22), Ui.dp(this, 22)));
            LinearLayout texts = new LinearLayout(this);
            texts.setOrientation(LinearLayout.VERTICAL);
            texts.setPadding(Ui.dp(this, 12), 0, 0, 0);
            texts.addView(Ui.label(this, displayTitle(n.name), 16));
            TextView when = Ui.label(this, "Edited " + DateUtils.getRelativeTimeSpanString(n.modified), 13);
            when.setAlpha(0.6f);
            texts.addView(when);
            row.addView(texts, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1));
            row.addView(Ui.icon(this, R.drawable.ic_trash, "Delete " + displayTitle(n.name), v ->
                    new AlertDialog.Builder(this)
                            .setMessage("Delete the annotation “" + displayTitle(n.name) + "”? The PDF is kept.")
                            .setPositiveButton("Delete", (d, w) -> {
                                deleteNote(n, parentId);
                                dialog[0].dismiss();
                                refresh();
                            })
                            .setNegativeButton(android.R.string.cancel, null).show()));
            row.setOnClickListener(v -> {
                dialog[0].dismiss();
                openNote(n.uri, parentId, n.name);
            });
            list.addView(row);
        }
        dialog[0] = new AlertDialog.Builder(this)
                .setTitle("Annotations of " + displayTitle(pdf.name))
                .setView(list)
                .setPositiveButton("New annotation", (d, w) -> newAnnotation(pdf.uri, pdf.name, parentId))
                .setNegativeButton(android.R.string.cancel, null)
                .show();
    }

    private void deleteNote(SafDocs.Entry note, String parentId) {
        SafDocs.Entry bg = SafDocs.child(getContentResolver(), tree, parentId, note.name + InkFile.BG_SUFFIX);
        if (bg != null) SafDocs.delete(getContentResolver(), bg.uri);
        SafDocs.delete(getContentResolver(), note.uri);
        Recents.remove(this, note.uri.toString());
    }

    /** Creates "&lt;pdf name&gt;.ink" in {@code parentId} linked to the PDF, then opens it. */
    @SuppressWarnings("deprecation")
    private void newAnnotation(Uri pdf, String pdfName, String parentId) {
        ProgressDialog pd = ProgressDialog.show(this, null, "Preparing " + pdfName + "…", true, false);
        io.execute(() -> {
            try {
                String name = SafDocs.uniqueName(getContentResolver(), tree, parentId,
                        Notes.sanitize(Notes.stripExt(pdfName)), InkFile.EXT);
                Uri note = createLinkedNote(pdf, pdfName, parentId, name);
                main.post(() -> {
                    pd.dismiss();
                    refresh();
                    openNote(note, parentId, name);
                });
            } catch (IOException | RuntimeException e) {
                main.post(() -> {
                    pd.dismiss();
                    toast("Could not open the PDF: " + e.getMessage());
                });
            }
        });
    }

    /** Writes a note whose pages show the PDF, which stays where it is. Off the UI thread. */
    private Uri createLinkedNote(Uri pdf, String pdfName, String parentId, String name) throws IOException {
        ContentResolver cr = getContentResolver();
        File tmp = new File(getCacheDir(), "import.pdf");
        SafDocs.copy(cr, pdf, tmp);
        Doc doc = Notes.pdfDoc(tmp);
        tmp.delete();
        doc.pdf = null;
        doc.pdfUri = pdf.toString();
        doc.pdfName = pdfName;
        doc.pdfPath = PdfLinks.realPath(this, pdf);
        Uri note = SafDocs.create(cr, tree, parentId, "application/octet-stream", name);
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        InkFile.write(doc, out, null);
        SafDocs.write(cr, note, out.toByteArray());
        return note;
    }

    /**
     * A PDF from outside the folder: keep a read grant and link to it where it is. If the
     * grant cannot be kept, copy the PDF into the folder and link to the copy.
     */
    private void annotateExternal(Uri pdf) {
        String pdfName = Notes.displayName(this, pdf, "document.pdf");
        String parent = currentId();
        try {
            getContentResolver().takePersistableUriPermission(pdf, Intent.FLAG_GRANT_READ_URI_PERMISSION);
            newAnnotation(pdf, pdfName, parent);
        } catch (SecurityException e) {
            io.execute(() -> {
                try {
                    String name = SafDocs.uniqueName(getContentResolver(), tree, parent,
                            Notes.sanitize(Notes.stripExt(pdfName)), ".pdf");
                    Uri copy = SafDocs.create(getContentResolver(), tree, parent, "application/pdf", name);
                    SafDocs.copy(getContentResolver(), pdf, copy);
                    main.post(() -> newAnnotation(copy, name, parent));
                } catch (IOException | RuntimeException ex) {
                    main.post(() -> toast("Could not open the PDF: " + ex.getMessage()));
                }
            });
        }
    }

    // ------------------------------------------------------------------ menus

    private void newMenu(View anchor) {
        if (tree == null) {
            chooseFolder();
            return;
        }
        PopupMenu m = new PopupMenu(this, anchor);
        m.getMenu().add(0, 1, 0, "New Markdown");
        m.getMenu().add(0, 2, 1, "New drawing");
        m.getMenu().add(0, 3, 2, "Annotate a PDF…");
        m.getMenu().add(0, 4, 3, "Import file…");
        m.getMenu().add(0, 5, 4, "New folder");
        m.setOnMenuItemClickListener(item -> {
            switch (item.getItemId()) {
                case 1:
                    prompt("New Markdown", "Untitled", name -> createMarkdown(name));
                    break;
                case 2:
                    createDrawing();
                    break;
                case 3: {
                    Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                    i.addCategory(Intent.CATEGORY_OPENABLE);
                    i.setType("application/pdf");
                    startActivityForResult(i, REQ_PDF);
                    break;
                }
                case 4: {
                    Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                    i.addCategory(Intent.CATEGORY_OPENABLE);
                    i.setType("*/*");
                    startActivityForResult(i, REQ_IMPORT);
                    break;
                }
                case 5:
                    prompt("New folder", "", name -> {
                        try {
                            SafDocs.create(getContentResolver(), tree, currentId(), DocumentsContract.Document.MIME_TYPE_DIR, Notes.sanitize(name));
                        } catch (IOException e) {
                            toast(e.getMessage());
                        }
                        refresh();
                    });
                    break;
                default:
                    break;
            }
            return true;
        });
        m.show();
    }

    private void createMarkdown(String title) {
        String base = Notes.sanitize(title);
        String parent = currentId();
        io.execute(() -> {
            try {
                String name = SafDocs.uniqueName(getContentResolver(), tree, parent, base, ".md");
                Uri uri = SafDocs.create(getContentResolver(), tree, parent, "text/markdown", name);
                SafDocs.write(getContentResolver(), uri, ("# " + Notes.stripExt(name) + "\n\n").getBytes(StandardCharsets.UTF_8));
                main.post(() -> openMarkdown(uri, name));
            } catch (IOException | RuntimeException e) {
                main.post(() -> toast(e.getMessage()));
            }
        });
    }

    private void createDrawing() {
        String base = "Drawing " + new SimpleDateFormat("yyyy-MM-dd HH.mm", Locale.getDefault()).format(new Date());
        String parent = currentId();
        io.execute(() -> {
            try {
                String name = SafDocs.uniqueName(getContentResolver(), tree, parent, base, InkFile.EXT);
                Uri uri = SafDocs.create(getContentResolver(), tree, parent, "application/octet-stream", name);
                ByteArrayOutputStream out = new ByteArrayOutputStream();
                InkFile.write(Doc.blank(), out, null);
                SafDocs.write(getContentResolver(), uri, out.toByteArray());
                main.post(() -> openNote(uri, parent, name));
            } catch (IOException | RuntimeException e) {
                main.post(() -> toast(e.getMessage()));
            }
        });
    }

    private void sortMenu(View anchor) {
        PopupMenu m = new PopupMenu(this, anchor);
        boolean byName = "name".equals(prefs.getString("sort", "modified"));
        m.getMenu().add(0, 1, 0, "Last modified").setCheckable(true).setChecked(!byName);
        m.getMenu().add(0, 2, 1, "Name").setCheckable(true).setChecked(byName);
        m.setOnMenuItemClickListener(item -> {
            prefs.edit().putString("sort", item.getItemId() == 2 ? "name" : "modified").apply();
            refresh();
            return true;
        });
        m.show();
    }

    private void moreMenu(View anchor) {
        PopupMenu m = new PopupMenu(this, anchor);
        m.getMenu().add(0, 1, 0, "Change folder…");
        m.getMenu().add(0, 3, 1, PdfLinks.hasFileAccess(this) ? "All files access: on" : "Allow all files access…");
        m.getMenu().add(0, 2, 2, "About");
        m.setOnMenuItemClickListener(item -> {
            if (item.getItemId() == 1) chooseFolder();
            else if (item.getItemId() == 3) {
                new AlertDialog.Builder(this).setTitle("All files access")
                        .setMessage("Annotations remember where their PDF is. With all files access, Markd opens "
                                + "a linked PDF by its path, so it is still found after the app is reinstalled. "
                                + "Without it, you may need to choose the PDF again.")
                        .setPositiveButton("Open settings", (d, w) -> PdfLinks.requestFileAccess(this))
                        .setNegativeButton(android.R.string.cancel, null).show();
            }
            else {
                new AlertDialog.Builder(this).setTitle("Markd")
                        .setMessage("Markdown, drawings and PDF annotation in one place.\n\n"
                                + "Drawings are .ink files in the Xournal++ file structure (rename to .xopp for desktop Xournal++). "
                                + "An annotated PDF is copied next to its note and never changed; Export as PDF writes a copy with your ink.\n\n"
                                + "Icons: Lucide (ISC). PDF engine: PdfBox-Android (Apache 2.0).")
                        .setPositiveButton(android.R.string.ok, null).show();
            }
            return true;
        });
        m.show();
    }

    private void itemMenu(View anchor, SafDocs.Entry e) {
        Kind k = kind(e);
        PopupMenu m = new PopupMenu(this, anchor);
        Set<String> pinned = new HashSet<>(prefs.getStringSet("pinnedDocs", Collections.emptySet()));
        boolean isPinned = pinned.contains(e.uri.toString());
        if (k == Kind.PDF) {
            m.getMenu().add(0, 7, 0, "New annotation");
            m.getMenu().add(0, 8, 0, "Annotations…");
        }
        if (k != Kind.DIR) m.getMenu().add(0, 1, 0, isPinned ? "Unpin" : "Pin to top");
        m.getMenu().add(0, 2, 1, "Rename");
        if (k != Kind.DIR) m.getMenu().add(0, 3, 2, "Duplicate");
        if (k == Kind.NOTE) m.getMenu().add(0, 4, 3, "Share as PDF");
        if (k != Kind.DIR) m.getMenu().add(0, 5, 4, k == Kind.NOTE ? "Share note file (.ink)" : "Share");
        m.getMenu().add(0, 6, 5, "Delete");
        String parent = parentOf(e);
        m.setOnMenuItemClickListener(item -> {
            switch (item.getItemId()) {
                case 7:
                    newAnnotation(e.uri, e.name, parent);
                    break;
                case 8:
                    io.execute(() -> {
                        List<SafDocs.Entry> notes = notesFor(e, parent);
                        main.post(() -> {
                            if (notes.isEmpty()) toast("No annotations yet. Tap the PDF to start one.");
                            else chooseAnnotation(e, parent, notes);
                        });
                    });
                    break;
                case 1:
                    if (isPinned) pinned.remove(e.uri.toString());
                    else pinned.add(e.uri.toString());
                    prefs.edit().putStringSet("pinnedDocs", pinned).apply();
                    refresh();
                    break;
                case 2:
                    prompt("Rename", k == Kind.DIR || k == Kind.PDF ? e.name : displayTitle(e.name), name -> rename(e, k, parent, name));
                    break;
                case 3:
                    duplicate(e, k, parent);
                    break;
                case 4:
                    sharePdf(e, parent);
                    break;
                case 5:
                    share(e, k);
                    break;
                case 6:
                    new AlertDialog.Builder(this)
                            .setMessage("Delete “" + displayTitle(e.name) + "”" + (k == Kind.DIR ? " and everything in it" : "")
                                    + "? This cannot be undone.")
                            .setPositiveButton("Delete", (d, w) -> {
                                if (k == Kind.NOTE) deleteNote(e, parent);
                                else {
                                    SafDocs.delete(getContentResolver(), e.uri);
                                    Recents.remove(this, e.uri.toString());
                                }
                                refresh();
                            })
                            .setNegativeButton(android.R.string.cancel, null).show();
                    break;
                default:
                    break;
            }
            return true;
        });
        m.show();
    }

    private void rename(SafDocs.Entry e, Kind k, String parent, String input) {
        String ext = k == Kind.MARKDOWN ? (e.name.contains(".") ? e.name.substring(e.name.lastIndexOf('.')) : ".md")
                : k == Kind.NOTE ? InkFile.EXT : "";
        String name = Notes.sanitize(input);
        if (!ext.isEmpty() && !name.toLowerCase(Locale.ROOT).endsWith(ext)) name += ext;
        try {
            if (SafDocs.child(getContentResolver(), tree, parent, name) != null) throw new IOException("That name is already used");
            SafDocs.Entry bg = k == Kind.NOTE ? SafDocs.child(getContentResolver(), tree, parent, e.name + InkFile.BG_SUFFIX) : null;
            Uri renamed = SafDocs.rename(getContentResolver(), e.uri, name);
            if (bg != null) SafDocs.rename(getContentResolver(), bg.uri, name + InkFile.BG_SUFFIX);
            Recents.rename(this, e.uri.toString(), renamed.toString(), name);
        } catch (IOException ex) {
            toast(ex.getMessage());
        }
        refresh();
    }

    private void duplicate(SafDocs.Entry e, Kind k, String parent) {
        io.execute(() -> {
            try {
                ContentResolver cr = getContentResolver();
                String ext = e.name.contains(".") ? e.name.substring(e.name.lastIndexOf('.')) : "";
                String name = SafDocs.uniqueName(cr, tree, parent, Notes.stripExt(e.name) + " copy", ext);
                Uri copy = SafDocs.create(cr, tree, parent, e.mime == null ? "application/octet-stream" : e.mime, name);
                SafDocs.copy(cr, e.uri, copy);
                if (k == Kind.NOTE) {
                    SafDocs.Entry bg = SafDocs.child(cr, tree, parent, e.name + InkFile.BG_SUFFIX);
                    if (bg != null) {
                        Uri bgCopy = SafDocs.create(cr, tree, parent, "application/pdf", name + InkFile.BG_SUFFIX);
                        SafDocs.copy(cr, bg.uri, bgCopy);
                    }
                }
                main.post(this::refresh);
            } catch (IOException | RuntimeException ex) {
                main.post(() -> toast(ex.getMessage()));
            }
        });
    }

    private void share(SafDocs.Entry e, Kind k) {
        Intent i = new Intent(Intent.ACTION_SEND);
        i.setType(k == Kind.MARKDOWN ? "text/markdown" : k == Kind.PDF ? "application/pdf" : "application/octet-stream");
        i.putExtra(Intent.EXTRA_STREAM, e.uri);
        i.setClipData(android.content.ClipData.newRawUri(e.name, e.uri));
        i.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
        startActivity(Intent.createChooser(i, "Share " + e.name));
    }

    @SuppressWarnings("deprecation")
    private void sharePdf(SafDocs.Entry e, String parent) {
        ProgressDialog pd = ProgressDialog.show(this, null, "Exporting PDF…", true, false);
        io.execute(() -> {
            try {
                File dir = new File(getCacheDir(), "share/export");
                dir.mkdirs();
                File note = new File(dir, e.name);
                SafDocs.copy(getContentResolver(), e.uri, note);
                SafDocs.Entry bg = SafDocs.child(getContentResolver(), tree, parent, e.name + InkFile.BG_SUFFIX);
                if (bg != null) SafDocs.copy(getContentResolver(), bg.uri, InkFile.bgPdfFor(note));
                Doc doc = InkFile.read(note);
                File out = new File(dir, Notes.sanitize(displayTitle(e.name)) + ".pdf");
                PdfExport.export(doc, out, null);
                main.post(() -> {
                    pd.dismiss();
                    ShareProvider.share(this, out, "application/pdf");
                });
            } catch (IOException | RuntimeException | OutOfMemoryError ex) {
                main.post(() -> {
                    pd.dismiss();
                    toast("Export failed: " + ex.getMessage());
                });
            }
        });
    }

    private interface NameCallback {
        void onName(String name);
    }

    private void prompt(String title, String initial, NameCallback cb) {
        EditText e = new EditText(this);
        e.setSingleLine(true);
        e.setText(initial);
        e.setSelectAllOnFocus(true);
        FrameLayout box = new FrameLayout(this);
        int pad = Ui.dp(this, 20);
        box.setPadding(pad, Ui.dp(this, 8), pad, 0);
        box.addView(e);
        new AlertDialog.Builder(this).setTitle(title).setView(box)
                .setPositiveButton(android.R.string.ok, (d, w) -> {
                    String s = e.getText().toString().trim();
                    if (!s.isEmpty()) cb.onName(s);
                })
                .setNegativeButton(android.R.string.cancel, null).show();
        e.requestFocus();
    }

    private void toast(String s) {
        Toast.makeText(this, s, Toast.LENGTH_LONG).show();
    }

    // ------------------------------------------------------------------ folder setup, import, intents

    private void chooseFolder() {
        Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        i.putExtra(DocumentsContract.EXTRA_INITIAL_URI,
                Uri.parse("content://com.android.externalstorage.documents/document/primary%3ADocuments"));
        i.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                | Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION);
        startActivityForResult(i, REQ_TREE);
    }

    private void onFolderChosen(Uri t) {
        try {
            getContentResolver().takePersistableUriPermission(t,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION | Intent.FLAG_GRANT_WRITE_URI_PERMISSION);
        } catch (SecurityException e) {
            toast("No write access to that folder");
            return;
        }
        tree = t;
        path.clear();
        prefs.edit().putString(KEY_TREE, t.toString()).apply();
        thumbs.evictAll();
        refresh();
        offerMigration();
        if (pendingImport != null) {
            Uri u = pendingImport;
            pendingImport = null;
            importUri(u, null);
        }
    }

    /** Notes made before the home screen existed live in the app's own folder; offer to copy them. */
    private void offerMigration() {
        File old = Notes.root(this);
        File[] files = old.listFiles((d, n) -> n.endsWith(InkFile.EXT));
        if (files == null || files.length == 0 || prefs.getBoolean("migrated", false)) return;
        new AlertDialog.Builder(this)
                .setTitle("Copy your notes here?")
                .setMessage(files.length + " note(s) from the old Notes & PDFs screen are stored inside the app. "
                        + "Copy them into this folder?")
                .setPositiveButton("Copy", (d, w) -> io.execute(() -> {
                    int ok = 0;
                    for (File f : files) {
                        try {
                            String parent = SafDocs.rootId(tree);
                            String name = SafDocs.uniqueName(getContentResolver(), tree, parent, Notes.title(f), InkFile.EXT);
                            Uri u = SafDocs.create(getContentResolver(), tree, parent, "application/octet-stream", name);
                            SafDocs.copy(getContentResolver(), f, u);
                            File bg = InkFile.bgPdfFor(f);
                            if (bg.isFile()) {
                                Uri b = SafDocs.create(getContentResolver(), tree, parent, "application/pdf", name + InkFile.BG_SUFFIX);
                                SafDocs.copy(getContentResolver(), bg, b);
                            }
                            ok++;
                        } catch (IOException | RuntimeException ignored) {
                            // keep going
                        }
                    }
                    int copied = ok;
                    main.post(() -> {
                        prefs.edit().putBoolean("migrated", true).apply();
                        toast("Copied " + copied + " of " + files.length + " notes. The originals are kept in the app.");
                        refresh();
                    });
                }))
                .setNegativeButton("Not now", null).show();
    }

    private void handleIntent(Intent intent) {
        if (intent == null) return;
        Uri uri = null;
        if (Intent.ACTION_VIEW.equals(intent.getAction())) uri = intent.getData();
        else if (Intent.ACTION_SEND.equals(intent.getAction())) uri = intent.getParcelableExtra(Intent.EXTRA_STREAM);
        if (uri == null) return;
        intent.setAction(null);
        if (tree == null) {
            pendingImport = uri;
            toast("Choose a folder for your files first");
            chooseFolder();
            return;
        }
        importUri(uri, intent.getType());
    }

    /** Copies a file from elsewhere into the current folder and opens it. */
    @SuppressWarnings("deprecation")
    private void importUri(Uri uri, String type) {
        String name = Notes.displayName(this, uri, "Imported");
        Kind k = "application/pdf".equals(type) ? Kind.PDF : kindOfName(name);
        if (k == null) {
            toast("Markd opens Markdown, PDF, .ink and .xopp files");
            return;
        }
        String parent = currentId();
        ProgressDialog pd = ProgressDialog.show(this, null, "Importing " + name + "…", true, false);
        io.execute(() -> {
            try {
                ContentResolver cr = getContentResolver();
                if (k == Kind.PDF) {
                    main.post(() -> {
                        pd.dismiss();
                        annotateExternal(uri);
                    });
                    return;
                }
                String ext = k == Kind.NOTE ? InkFile.EXT : name.contains(".") ? name.substring(name.lastIndexOf('.')) : ".md";
                String target = SafDocs.uniqueName(cr, tree, parent, Notes.sanitize(Notes.stripExt(name)), ext);
                Uri out = SafDocs.create(cr, tree, parent, k == Kind.NOTE ? "application/octet-stream" : "text/markdown", target);
                SafDocs.copy(cr, uri, out);
                main.post(() -> {
                    pd.dismiss();
                    refresh();
                    if (k == Kind.NOTE) openNote(out, parent, target);
                    else openMarkdown(out, target);
                });
            } catch (IOException | RuntimeException e) {
                main.post(() -> {
                    pd.dismiss();
                    toast("Could not import: " + e.getMessage());
                });
            }
        });
    }

    /** Import file → a PDF: copy it into the folder; tapping it later starts an annotation. */
    private void importPdfCopy(Uri pdf) {
        String pdfName = Notes.displayName(this, pdf, "document.pdf");
        String parent = currentId();
        io.execute(() -> {
            try {
                String name = SafDocs.uniqueName(getContentResolver(), tree, parent, Notes.sanitize(Notes.stripExt(pdfName)), ".pdf");
                Uri copy = SafDocs.create(getContentResolver(), tree, parent, "application/pdf", name);
                SafDocs.copy(getContentResolver(), pdf, copy);
                main.post(() -> {
                    toast("Imported " + name);
                    refresh();
                });
            } catch (IOException | RuntimeException e) {
                main.post(() -> toast("Could not import: " + e.getMessage()));
            }
        });
    }

    @Override
    protected void onActivityResult(int req, int res, Intent data) {
        super.onActivityResult(req, res, data);
        if (res != RESULT_OK || data == null || data.getData() == null) {
            if (req == REQ_TREE) pendingImport = null;
            return;
        }
        if (req == REQ_TREE) onFolderChosen(data.getData());
        else if (req == REQ_PDF) annotateExternal(data.getData());
        else if (req == REQ_IMPORT) {
            Uri u = data.getData();
            if (kindOfName(Notes.displayName(this, u, "")) == Kind.PDF) importPdfCopy(u);
            else importUri(u, null);
        }
    }

    // ------------------------------------------------------------------ thumbnails

    private Bitmap loadThumb(SafDocs.Entry e, Kind k, String parent) {
        ContentResolver cr = getContentResolver();
        try {
            switch (k) {
                case NOTE: {
                    Bitmap b = InkFile.readPreview(cr.openInputStream(e.uri));
                    if (b != null) return b;
                    String[] link = linkOf(e);
                    if (link != null && link[0] != null) {
                        try {
                            return renderPdf(Uri.parse(link[0]));
                        } catch (IOException | RuntimeException ignored) {
                            // grant gone (e.g. after reinstall); try the saved path
                        }
                    }
                    File byPath = link == null || link.length < 3 ? null : PdfLinks.readable(link[2]);
                    if (byPath != null) return renderPdf(Uri.fromFile(byPath));
                    SafDocs.Entry bg = SafDocs.child(cr, tree, parent, e.name + InkFile.BG_SUFFIX);
                    return bg == null ? null : renderPdf(bg.uri);
                }
                case PDF:
                    return renderPdf(e.uri);
                case MARKDOWN:
                    return renderMarkdown(cr, e);
                default:
                    return null;
            }
        } catch (IOException | RuntimeException | OutOfMemoryError ex) {
            return null;
        }
    }

    private Bitmap renderPdf(Uri uri) throws IOException {
        try (ParcelFileDescriptor fd = "file".equals(uri.getScheme())
                ? ParcelFileDescriptor.open(new File(uri.getPath()), ParcelFileDescriptor.MODE_READ_ONLY)
                : getContentResolver().openFileDescriptor(uri, "r");
             PdfRenderer r = new PdfRenderer(fd);
             PdfRenderer.Page p = r.openPage(0)) {
            int w = 300, h = Math.max(1, Math.round(300f * p.getHeight() / p.getWidth()));
            Bitmap out = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888);
            out.eraseColor(Color.WHITE);
            p.render(out, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY);
            return out;
        }
    }

    /** A small page showing the first lines of a Markdown file. */
    private Bitmap renderMarkdown(ContentResolver cr, SafDocs.Entry e) throws IOException {
        char[] buf = new char[2400];
        int n;
        try (InputStream in = cr.openInputStream(e.uri);
             Reader r = new InputStreamReader(in, StandardCharsets.UTF_8)) {
            n = Math.max(0, r.read(buf));
        }
        String text = new String(buf, 0, n);
        int w = 300, h = 400, pad = 22;
        Bitmap b = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888);
        b.eraseColor(Color.WHITE);
        Canvas c = new Canvas(b);
        Paint head = new Paint(Paint.ANTI_ALIAS_FLAG);
        head.setColor(0xff1a1a1a);
        head.setTextSize(22);
        head.setTypeface(Typeface.DEFAULT_BOLD);
        Paint body = new Paint(Paint.ANTI_ALIAS_FLAG);
        body.setColor(0xff555555);
        body.setTextSize(15);
        float y = pad + 20;
        for (String raw : text.split("\n")) {
            String line = raw.trim();
            if (line.isEmpty()) {
                y += 8;
                continue;
            }
            boolean heading = line.startsWith("#");
            line = line.replaceAll("^#+\\s*", "").replaceAll("[*_`>]", "").replaceAll("!?\\[([^\\]]*)]\\([^)]*\\)", "$1");
            Paint p = heading ? head : body;
            // wrap
            while (!line.isEmpty() && y < h - pad) {
                int count = p.breakText(line, true, w - 2 * pad, null);
                if (count <= 0) break;
                c.drawText(line, 0, count, pad, y, p);
                line = line.substring(count).trim();
                y += heading ? 28 : 20;
            }
            if (y >= h - pad) break;
        }
        return b;
    }

    private final class Adapter extends BaseAdapter {
        @Override
        public int getCount() {
            return items.size();
        }

        @Override
        public Object getItem(int i) {
            return items.get(i);
        }

        @Override
        public long getItemId(int i) {
            return i;
        }

        @Override
        public View getView(int pos, View convert, ViewGroup parent) {
            Holder h;
            if (convert == null) {
                h = new Holder();
                LinearLayout card = new LinearLayout(HomeActivity.this);
                card.setOrientation(LinearLayout.VERTICAL);
                card.setPadding(Ui.dp(HomeActivity.this, 6), Ui.dp(HomeActivity.this, 6), Ui.dp(HomeActivity.this, 6), Ui.dp(HomeActivity.this, 8));
                card.setBackground(Ui.ripple(HomeActivity.this, 0));
                FrameLayout thumbBox = new FrameLayout(HomeActivity.this);
                h.image = new ImageView(HomeActivity.this);
                h.image.setScaleType(ImageView.ScaleType.FIT_CENTER);
                thumbBox.addView(h.image, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
                h.badge = Ui.label(HomeActivity.this, "", 11);
                h.badge.setTextColor(Color.WHITE);
                int bp = Ui.dp(HomeActivity.this, 6);
                h.badge.setPadding(bp, Ui.dp(HomeActivity.this, 2), bp, Ui.dp(HomeActivity.this, 2));
                FrameLayout.LayoutParams blp = new FrameLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT, Gravity.BOTTOM | Gravity.START);
                blp.setMargins(Ui.dp(HomeActivity.this, 6), 0, 0, Ui.dp(HomeActivity.this, 6));
                thumbBox.addView(h.badge, blp);
                h.star = new ImageView(HomeActivity.this);
                h.star.setImageResource(R.drawable.ic_star_filled);
                h.star.setColorFilter(0xfff4b400);
                FrameLayout.LayoutParams slp = new FrameLayout.LayoutParams(Ui.dp(HomeActivity.this, 20), Ui.dp(HomeActivity.this, 20), Gravity.TOP | Gravity.END);
                slp.setMargins(0, Ui.dp(HomeActivity.this, 6), Ui.dp(HomeActivity.this, 6), 0);
                thumbBox.addView(h.star, slp);
                card.addView(thumbBox, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, Ui.dp(HomeActivity.this, 190)));
                h.title = Ui.label(HomeActivity.this, "", 14);
                h.title.setPadding(Ui.dp(HomeActivity.this, 2), Ui.dp(HomeActivity.this, 6), 0, 0);
                card.addView(h.title);
                h.sub = Ui.label(HomeActivity.this, "", 12);
                h.sub.setAlpha(0.6f);
                h.sub.setPadding(Ui.dp(HomeActivity.this, 2), 0, 0, 0);
                card.addView(h.sub);
                card.setTag(h);
                convert = card;
            } else {
                h = (Holder) convert.getTag();
            }
            SafDocs.Entry e = items.get(pos);
            Kind k = kind(e);
            boolean pinned = prefs.getStringSet("pinnedDocs", Collections.emptySet()).contains(e.uri.toString());
            h.star.setVisibility(pinned ? View.VISIBLE : View.GONE);
            h.title.setText(displayTitle(e.name));
            if (k == Kind.DIR) {
                h.sub.setText("Folder");
                h.badge.setVisibility(View.GONE);
                h.image.setImageResource(R.drawable.ic_folder);
                h.image.setBackground(null);
                int p = Ui.dp(HomeActivity.this, 40);
                h.image.setPadding(p, p, p, p);
                return convert;
            }
            int linked = k == Kind.PDF ? linkedCount(e) : 0;
            String label = k == Kind.MARKDOWN ? "MD" : k == Kind.PDF ? (linked > 0 ? "PDF · " + linked + " ink" : "PDF") : "INK";
            int color = k == Kind.MARKDOWN ? 0xff185abd : k == Kind.PDF ? 0xffd93025 : 0xff188038;
            h.badge.setText(label);
            GradientDrawable bd = new GradientDrawable();
            bd.setCornerRadius(Ui.dp(HomeActivity.this, 6));
            bd.setColor(color);
            h.badge.setBackground(bd);
            h.badge.setVisibility(View.VISIBLE);
            h.sub.setText(DateUtils.getRelativeTimeSpanString(e.modified));
            h.image.setPadding(0, 0, 0, 0);
            GradientDrawable paper = new GradientDrawable();
            paper.setColor(Color.WHITE);
            paper.setStroke(Ui.dp(HomeActivity.this, 1), 0x22000000);
            paper.setCornerRadius(Ui.dp(HomeActivity.this, 4));
            String key = e.uri + "|" + e.modified;
            Bitmap b = thumbs.get(key);
            h.image.setImageBitmap(b);
            h.image.setBackground(b == null ? paper : null);
            if (b == null && loading.add(key)) {
                String parentId = parentOf(e);
                io.execute(() -> {
                    Bitmap t = loadThumb(e, k, parentId);
                    main.post(() -> {
                        loading.remove(key);
                        if (t != null) {
                            thumbs.put(key, t);
                            notifyDataSetChanged();
                        }
                    });
                });
            }
            return convert;
        }
    }

    private static final class Holder {
        ImageView image, star;
        TextView title, sub, badge;
    }
}
