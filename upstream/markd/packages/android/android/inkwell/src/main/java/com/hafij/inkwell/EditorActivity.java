package com.hafij.inkwell;

import android.app.Activity;
import android.app.AlertDialog;
import android.app.ProgressDialog;
import android.content.Intent;
import android.content.SharedPreferences;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.drawable.BitmapDrawable;
import android.graphics.drawable.GradientDrawable;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.text.InputType;
import android.view.Gravity;
import android.view.Menu;
import android.view.MenuItem;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.widget.CheckBox;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.ImageButton;
import android.widget.LinearLayout;
import android.widget.NumberPicker;
import android.widget.PopupMenu;
import android.widget.SeekBar;
import android.widget.TextView;
import android.widget.Toast;

import com.hafij.inkwell.editor.CanvasView;
import com.hafij.inkwell.editor.Shapes;
import com.hafij.inkwell.editor.Tools;
import com.hafij.inkwell.editor.Undo;
import com.hafij.inkwell.io.InkFile;
import com.hafij.inkwell.io.SvgSketch;
import com.hafij.inkwell.model.Doc;
import com.hafij.inkwell.model.TextEl;
import com.hafij.inkwell.pdf.PdfExport;
import com.hafij.inkwell.pdf.PdfPages;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.util.EnumMap;
import java.util.Map;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** The note editor: toolbars around a {@link CanvasView}. */
public class EditorActivity extends Activity implements CanvasView.Host {
    public static final String EXTRA_PATH = "path";
    /** Note in the library folder (SAF): document uri, tree uri, parent document id, file name. */
    public static final String EXTRA_DOC_URI = "docUri";
    public static final String EXTRA_TREE_URI = "treeUri";
    public static final String EXTRA_PARENT_ID = "parentId";
    public static final String EXTRA_NAME = "name";
    private static final int REQ_IMAGE = 1, REQ_EXPORT = 2, REQ_LOCATE_PDF = 3;

    private final ExecutorService io = Executors.newSingleThreadExecutor();
    private final Handler main = new Handler(Looper.getMainLooper());
    private SharedPreferences prefs;
    private File note;
    private Doc doc;
    private Undo undo;
    private final Tools tools = new Tools();
    private PdfPages pdf;
    private CanvasView canvas;
    private boolean dirty;
    private boolean saving;
    /** Set when editing a note in the library folder: the local file is a working copy. */
    private Uri docUri, treeUri;
    private String parentId;
    /** Set while loading when the PDF was found somewhere else: the link to update. */
    private volatile Uri foundPdf;
    private volatile String foundPath;
    private boolean waitingForAccess;

    private TextView titleView, pageView;
    private ImageButton undoBtn, redoBtn;
    private final Map<Tools.Tool, ImageButton> toolButtons = new EnumMap<>(Tools.Tool.class);
    private final ImageButton[] sizeButtons = new ImageButton[5];
    private LinearLayout colorRow;
    private LinearLayout selectionBar;
    private ImageButton styleBtn, fillBtn, shapeBtn;
    private TextView opacityBtn;
    private View sizeDivider, styleDivider;

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        Inkwell.init(this);
        prefs = getSharedPreferences("inkwell", MODE_PRIVATE);
        tools.load(prefs);
        if (isSketch()) {
            note = prepareSketch();
        } else if (getIntent().getStringExtra(EXTRA_DOC_URI) != null) {
            docUri = Uri.parse(getIntent().getStringExtra(EXTRA_DOC_URI));
            treeUri = Uri.parse(getIntent().getStringExtra(EXTRA_TREE_URI));
            parentId = getIntent().getStringExtra(EXTRA_PARENT_ID);
            String name = getIntent().getStringExtra(EXTRA_NAME);
            File dir = new File(new File(getCacheDir(), "share/work"), Integer.toHexString(docUri.toString().hashCode()));
            dir.mkdirs();
            note = new File(dir, name == null ? "Note" + InkFile.EXT : name);
        } else {
            String path = getIntent().getStringExtra(EXTRA_PATH);
            note = path == null ? null : new File(path);
        }
        if (note == null) {
            finish();
            return;
        }
        if (prefs.getBoolean("keepScreenOn", false)) getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        buildUi();
        load();
    }

    // ------------------------------------------------------------------ loading

    private void load() {
        titleView.setText(isSketch() ? "Sketch" : Notes.title(note));
        io.execute(() -> {
            try {
                if (docUri != null) pullFromFolder();
                Doc d = InkFile.read(note);
                main.post(() -> onLoaded(d));
            } catch (IOException | RuntimeException e) {
                main.post(() -> {
                    Toast.makeText(this, "Could not open: " + e.getMessage(), Toast.LENGTH_LONG).show();
                    finish();
                });
            }
        });
    }

    private void onLoaded(Doc d) {
        doc = d;
        undo = new Undo(doc);
        pdf = new PdfPages(canvas);
        openPdf();
        canvas.setup(doc, undo, tools, pdf, this);
        updateUndo();
        if (foundPath != null && doc.pdf != null) {
            doc.pdfPath = foundPath;
            foundPath = null;
            onEdited();
            Toast.makeText(this, "Found the PDF at " + doc.pdfPath, Toast.LENGTH_LONG).show();
        }
        if (foundPdf != null && doc.pdf != null) {
            // Found by name after the old link stopped working: remember the new place.
            doc.pdfUri = foundPdf.toString();
            doc.pdfPath = PdfLinks.realPath(this, foundPdf);
            foundPdf = null;
            onEdited();
            Toast.makeText(this, "Found " + new File(doc.pdfName == null ? "the PDF" : doc.pdfName).getName()
                    + " in your folder", Toast.LENGTH_SHORT).show();
        }
        if (doc.pdf != null && doc.pdfUri != null && doc.pdfPath == null) {
            // Notes from 0.24.1 have no path yet; add it while the link still works.
            String path = PdfLinks.realPath(this, Uri.parse(doc.pdfUri));
            if (path != null) {
                doc.pdfPath = path;
                dirty = true;
                save(false, null);
            }
        }
        if (doc.pdfMissing) askForPdf();
    }

    private void openPdf() {
        if (doc.pdf == null) return;
        try {
            pdf.open(doc.pdf);
        } catch (IOException e) {
            doc.pdfMissing = true;
            Toast.makeText(this, "Cannot read the background PDF: " + e.getMessage(), Toast.LENGTH_LONG).show();
        }
    }

    private void askForPdf() {
        String name = doc.pdfName == null ? "the background PDF" : new File(doc.pdfName).getName();
        boolean needAccess = !PdfLinks.hasFileAccess(this);
        String where = doc.pdfPath != null ? "\n\nIt was at " + doc.pdfPath + "." : "";
        String msg = needAccess
                ? "This note annotates " + name + ", and Markd can no longer open its link." + where
                + "\n\nAllow all files access and Markd finds the PDF on this device by itself, "
                + "or choose the PDF yourself."
                : "This note annotates " + name + ", which was not found. It may have been moved, renamed or deleted."
                + where + "\n\nChoose the PDF to link it again.";
        AlertDialog.Builder b = new AlertDialog.Builder(this)
                .setTitle("PDF not found")
                .setMessage(msg)
                .setPositiveButton("Choose PDF", (dl, w) -> {
                    Intent i = new Intent(Intent.ACTION_OPEN_DOCUMENT);
                    i.addCategory(Intent.CATEGORY_OPENABLE);
                    i.setType("application/pdf");
                    startActivityForResult(i, REQ_LOCATE_PDF);
                })
                .setNegativeButton("Not now", null);
        if (needAccess) {
            b.setNeutralButton("Allow access", (dl, w) -> {
                waitingForAccess = true;
                PdfLinks.requestFileAccess(this);
            });
        }
        b.show();
    }

    @Override
    protected void onResume() {
        super.onResume();
        if (waitingForAccess && PdfLinks.hasFileAccess(this)) {
            // Reopen so the PDF is found by its path.
            waitingForAccess = false;
            save(false, this::recreate);
        }
    }

    // ------------------------------------------------------------------ UI

    private void buildUi() {
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setBackgroundColor(Ui.surface(this));
        root.setFitsSystemWindows(true);

        // Top bar
        LinearLayout top = row();
        top.addView(Ui.icon(this, R.drawable.ic_back, "Back", v -> onBackPressed()));
        titleView = Ui.label(this, "", 17);
        if (!isSketch()) titleView.setOnClickListener(v -> renameDialog());
        top.addView(titleView, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.MATCH_PARENT, 1));
        pageView = Ui.label(this, "", 13);
        pageView.setPadding(Ui.dp(this, 8), 0, Ui.dp(this, 8), 0);
        pageView.setOnClickListener(v -> gotoDialog());
        pageView.setBackground(Ui.ripple(this, 0));
        top.addView(pageView, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, Ui.dp(this, 44)));
        undoBtn = Ui.icon(this, R.drawable.ic_edit_undo, "Undo", v -> {
            if (undo != null && undo.undo()) afterUndo();
        });
        redoBtn = Ui.icon(this, R.drawable.ic_edit_redo, "Redo", v -> {
            if (undo != null && undo.redo()) afterUndo();
        });
        top.addView(undoBtn);
        top.addView(redoBtn);
        if (isSketch()) {
            ImageButton done = Ui.icon(this, R.drawable.ic_check, "Insert sketch", v -> finishSketch());
            done.setBackground(Ui.ripple(this, Ui.withAlpha(Ui.accent(this), 0x40)));
            top.addView(done);
        } else {
            top.addView(Ui.icon(this, R.drawable.ic_page_add, "Add page", v -> addPageAfter()));
        }
        ImageButton more = Ui.icon(this, R.drawable.ic_more, "More", null);
        more.setOnClickListener(v -> showMenu(more));
        top.addView(more);
        root.addView(top);

        // Tools
        LinearLayout toolRow = row();
        addTool(toolRow, Tools.Tool.PEN, R.drawable.ic_tool_pencil, "Pen");
        addTool(toolRow, Tools.Tool.HIGHLIGHTER, R.drawable.ic_tool_highlighter, "Highlighter");
        addTool(toolRow, Tools.Tool.ERASER, R.drawable.ic_tool_eraser, "Eraser");
        addTool(toolRow, Tools.Tool.TEXT, R.drawable.ic_tool_text, "Text");
        addTool(toolRow, Tools.Tool.SHAPE, shapeIcon(), "Shapes");
        shapeBtn = toolButtons.get(Tools.Tool.SHAPE);
        addTool(toolRow, Tools.Tool.SELECT_LASSO, R.drawable.ic_select_lasso, "Lasso select");
        addTool(toolRow, Tools.Tool.SELECT_RECT, R.drawable.ic_select_rect, "Rectangle select");
        addTool(toolRow, Tools.Tool.HAND, R.drawable.ic_hand, "Hand");
        toolRow.addView(Ui.icon(this, R.drawable.ic_tool_image, "Insert image", v -> pickImage()));
        sizeDivider = divider();
        toolRow.addView(sizeDivider);
        for (int i = 0; i < 5; i++) {
            final int k = i;
            ImageButton b = Ui.icon(this, 0, "Size " + (i + 1), v -> {
                tools.setSizeIndex(k);
                refreshTools();
            });
            b.setImageDrawable(dotDrawable(3 + i * 3));
            sizeButtons[i] = b;
            toolRow.addView(b);
        }
        styleDivider = divider();
        toolRow.addView(styleDivider);
        styleBtn = Ui.icon(this, R.drawable.ic_line_style_plain, "Line style", null);
        styleBtn.setOnClickListener(v -> styleMenu(styleBtn));
        toolRow.addView(styleBtn);
        fillBtn = Ui.icon(this, R.drawable.ic_fill, "Fill (long-press for fill opacity)", v -> {
            tools.fill = !tools.fill;
            refreshTools();
        });
        fillBtn.setOnLongClickListener(v -> {
            fillOpacityDialog();
            return true;
        });
        toolRow.addView(fillBtn);
        opacityBtn = Ui.label(this, "", 13);
        opacityBtn.setGravity(Gravity.CENTER);
        opacityBtn.setContentDescription("Opacity");
        opacityBtn.setTooltipText("Opacity");
        opacityBtn.setBackground(Ui.ripple(this, 0));
        opacityBtn.setOnClickListener(v -> opacityDialog());
        toolRow.addView(opacityBtn, new LinearLayout.LayoutParams(Ui.dp(this, 52), Ui.dp(this, 44)));
        root.addView(scroll(toolRow));

        // Colours
        colorRow = row();
        root.addView(scroll(colorRow));
        buildColors();

        // Canvas + selection bar
        FrameLayout frame = new FrameLayout(this);
        canvas = new CanvasView(this);
        frame.addView(canvas, new FrameLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        selectionBar = row();
        GradientDrawable pill = new GradientDrawable();
        pill.setCornerRadius(Ui.dp(this, 24));
        pill.setColor(Ui.surface(this));
        pill.setStroke(Ui.dp(this, 1), 0x33808080);
        selectionBar.setBackground(pill);
        selectionBar.setElevation(Ui.dp(this, 6));
        selectionBar.setPadding(Ui.dp(this, 6), 0, Ui.dp(this, 6), 0);
        selectionBar.addView(Ui.icon(this, R.drawable.ic_trash, "Delete", v -> canvas.deleteSelection()));
        selectionBar.addView(Ui.icon(this, R.drawable.ic_copy_page, "Duplicate", v -> canvas.duplicateSelection()));
        selectionBar.addView(Ui.icon(this, R.drawable.ic_edit_copy, "Copy", v -> {
            canvas.copySelection();
            Toast.makeText(this, "Copied", Toast.LENGTH_SHORT).show();
        }));
        selectionBar.addView(Ui.icon(this, R.drawable.ic_edit_cut, "Cut", v -> canvas.cutSelection()));
        selectionBar.addView(Ui.icon(this, R.drawable.ic_palette, "Apply current colour",
                v -> canvas.recolorSelection(tools.penColor)));
        selectionBar.addView(Ui.icon(this, R.drawable.ic_move_up, "Bring to front", v -> canvas.bringSelectionToFront()));
        selectionBar.addView(Ui.icon(this, R.drawable.ic_close, "Deselect", v -> canvas.clearSelection()));
        FrameLayout.LayoutParams lp = new FrameLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, Ui.dp(this, 48),
                Gravity.BOTTOM | Gravity.CENTER_HORIZONTAL);
        lp.bottomMargin = Ui.dp(this, 16);
        selectionBar.setVisibility(View.GONE);
        frame.addView(selectionBar, lp);
        root.addView(frame, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1));

        setContentView(root);
        refreshTools();
    }

    private LinearLayout row() {
        LinearLayout r = new LinearLayout(this);
        r.setOrientation(LinearLayout.HORIZONTAL);
        r.setGravity(Gravity.CENTER_VERTICAL);
        r.setPadding(Ui.dp(this, 4), 0, Ui.dp(this, 4), 0);
        return r;
    }

    private HorizontalScrollView scroll(View child) {
        HorizontalScrollView s = new HorizontalScrollView(this);
        s.setHorizontalScrollBarEnabled(false);
        s.addView(child);
        return s;
    }

    private View divider() {
        View v = new View(this);
        v.setBackgroundColor(0x33808080);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(Ui.dp(this, 1), Ui.dp(this, 26));
        lp.leftMargin = lp.rightMargin = Ui.dp(this, 6);
        v.setLayoutParams(lp);
        return v;
    }

    private void addTool(LinearLayout row, Tools.Tool t, int icon, String name) {
        ImageButton b = Ui.icon(this, icon, name, v -> {
            if (tools.tool == t) {
                toolOptions(t, v);
            } else {
                tools.tool = t;
                if (t != Tools.Tool.SELECT_LASSO && t != Tools.Tool.SELECT_RECT) canvas.clearSelection();
                refreshTools();
            }
        });
        toolButtons.put(t, b);
        row.addView(b);
    }

    private BitmapDrawable dotDrawable(int diameterDp) {
        int size = Ui.dp(this, 24);
        Bitmap b = Bitmap.createBitmap(size, size, Bitmap.Config.ARGB_8888);
        Canvas c = new Canvas(b);
        Paint p = new Paint(Paint.ANTI_ALIAS_FLAG);
        p.setColor(Ui.text(this));
        c.drawCircle(size / 2f, size / 2f, Ui.dp(this, diameterDp) / 2f, p);
        return new BitmapDrawable(getResources(), b);
    }

    private void buildColors() {
        colorRow.removeAllViews();
        int sel = tools.currentColor() | 0xff000000;
        for (int c : Tools.PALETTE) colorRow.addView(swatch(c, (c | 0xff000000) == sel));
        boolean custom = true;
        for (int c : Tools.PALETTE) if ((c | 0xff000000) == sel) custom = false;
        if (custom) colorRow.addView(swatch(sel, true));
        colorRow.addView(Ui.icon(this, R.drawable.ic_palette, "Custom colour",
                v -> ColorPicker.show(this, tools.currentColor(), col -> {
                    tools.setCurrentColor(col | 0xff000000);
                    if (canvas.hasSelection()) canvas.recolorSelection(col | 0xff000000);
                    refreshTools();
                })));
    }

    private View swatch(int color, boolean selected) {
        View v = new View(this);
        GradientDrawable d = new GradientDrawable();
        d.setShape(GradientDrawable.OVAL);
        d.setColor(color);
        d.setStroke(Ui.dp(this, selected ? 3 : 1), selected ? Ui.accent(this) : 0x55808080);
        v.setBackground(d);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(Ui.dp(this, 28), Ui.dp(this, 28));
        lp.setMargins(Ui.dp(this, 5), Ui.dp(this, 6), Ui.dp(this, 5), Ui.dp(this, 6));
        v.setLayoutParams(lp);
        v.setContentDescription(String.format("Colour #%06X", color & 0xffffff));
        v.setOnClickListener(x -> {
            if (tools.tool == Tools.Tool.ERASER || tools.tool == Tools.Tool.HAND
                    || tools.tool == Tools.Tool.SELECT_LASSO || tools.tool == Tools.Tool.SELECT_RECT) {
                if (canvas.hasSelection()) {
                    canvas.recolorSelection(color);
                    tools.penColor = color;
                } else {
                    tools.tool = Tools.Tool.PEN;
                    tools.penColor = color;
                }
            } else {
                tools.setCurrentColor(color);
            }
            refreshTools();
        });
        return v;
    }

    private int shapeIcon() {
        switch (tools.shape) {
            case Shapes.RECT: return R.drawable.ic_draw_rect;
            case Shapes.ELLIPSE: return R.drawable.ic_draw_ellipse;
            case Shapes.ARROW:
            case Shapes.DOUBLE_ARROW: return R.drawable.ic_draw_arrow;
            case Shapes.AXES: return R.drawable.ic_draw_coordinate_system;
            default: return R.drawable.ic_draw_line;
        }
    }

    private void refreshTools() {
        int accent = Ui.accent(this);
        for (Map.Entry<Tools.Tool, ImageButton> e : toolButtons.entrySet()) {
            e.getValue().setBackground(Ui.ripple(this, e.getKey() == tools.tool ? Ui.withAlpha(accent, 0x40) : 0));
        }
        if (shapeBtn != null) shapeBtn.setImageResource(shapeIcon());
        boolean sized = tools.tool == Tools.Tool.PEN || tools.tool == Tools.Tool.HIGHLIGHTER
                || tools.tool == Tools.Tool.ERASER || tools.tool == Tools.Tool.SHAPE;
        for (int i = 0; i < 5; i++) {
            sizeButtons[i].setVisibility(sized ? View.VISIBLE : View.GONE);
            sizeButtons[i].setBackground(Ui.ripple(this, sized && tools.sizeIndex() == i ? Ui.withAlpha(accent, 0x40) : 0));
        }
        boolean pen = tools.tool == Tools.Tool.PEN || tools.tool == Tools.Tool.SHAPE;
        sizeDivider.setVisibility(sized ? View.VISIBLE : View.GONE);
        styleDivider.setVisibility(pen ? View.VISIBLE : View.GONE);
        styleBtn.setVisibility(pen ? View.VISIBLE : View.GONE);
        boolean opaque = pen || tools.tool == Tools.Tool.HIGHLIGHTER;
        opacityBtn.setVisibility(opaque ? View.VISIBLE : View.GONE);
        opacityBtn.setText(percent(tools.tool == Tools.Tool.HIGHLIGHTER ? tools.hlAlpha : tools.penAlpha));
        fillBtn.setVisibility(pen ? View.VISIBLE : View.GONE);
        fillBtn.setBackground(Ui.ripple(this, tools.fill ? Ui.withAlpha(accent, 0x40) : 0));
        int[] styleIcons = {R.drawable.ic_line_style_plain, R.drawable.ic_line_style_dash,
                R.drawable.ic_line_style_dash_dot, R.drawable.ic_line_style_dot};
        styleBtn.setImageResource(styleIcons[tools.penStyle]);
        buildColors();
        tools.save(prefs);
    }

    private void toolOptions(Tools.Tool t, View anchor) {
        PopupMenu m = new PopupMenu(this, anchor);
        Menu menu = m.getMenu();
        switch (t) {
            case PEN:
                menu.add(0, 1, 0, "Shape recogniser").setCheckable(true).setChecked(tools.recognizer);
                menu.add(0, 2, 0, "Pressure sensitivity").setCheckable(true).setChecked(tools.pressure);
                menu.add(0, 3, 0, "Opacity… (" + percent(tools.penAlpha) + ")");
                menu.add(0, 4, 0, "Fill opacity… (" + percent(tools.fillAlpha) + ")");
                m.setOnMenuItemClickListener(i -> {
                    if (i.getItemId() == 1) tools.recognizer = !tools.recognizer;
                    else if (i.getItemId() == 2) tools.pressure = !tools.pressure;
                    else if (i.getItemId() == 3) opacityDialog();
                    else fillOpacityDialog();
                    refreshTools();
                    return true;
                });
                break;
            case ERASER:
                menu.add(0, 1, 0, "Erase touched part").setCheckable(true).setChecked(!tools.eraseWhole);
                menu.add(0, 2, 0, "Erase whole strokes").setCheckable(true).setChecked(tools.eraseWhole);
                m.setOnMenuItemClickListener(i -> {
                    tools.eraseWhole = i.getItemId() == 2;
                    refreshTools();
                    return true;
                });
                break;
            case HIGHLIGHTER:
                menu.add(0, 1, 0, "Opacity… (" + percent(tools.hlAlpha) + ")");
                menu.add(0, 2, 0, "Fill opacity… (" + percent(tools.fillAlpha) + ")");
                m.setOnMenuItemClickListener(i -> {
                    if (i.getItemId() == 1) opacityDialog();
                    else fillOpacityDialog();
                    return true;
                });
                break;
            case SHAPE:
                String[] names = {"Line", "Rectangle", "Ellipse", "Arrow", "Double arrow", "Coordinate system"};
                for (int i = 0; i < names.length; i++) {
                    menu.add(0, i, i, names[i]).setCheckable(true).setChecked(tools.shape == i);
                }
                menu.add(1, 10, 10, "Opacity… (" + percent(tools.penAlpha) + ")");
                menu.add(1, 11, 11, "Fill opacity… (" + percent(tools.fillAlpha) + ")");
                m.setOnMenuItemClickListener(i -> {
                    if (i.getItemId() == 10) opacityDialog();
                    else if (i.getItemId() == 11) fillOpacityDialog();
                    else tools.shape = i.getItemId();
                    refreshTools();
                    return true;
                });
                break;
            case TEXT:
                menu.add(0, 1, 0, "Text size…");
                m.setOnMenuItemClickListener(i -> {
                    textSizeDialog();
                    return true;
                });
                break;
            default:
                return;
        }
        m.show();
    }

    private void styleMenu(View anchor) {
        PopupMenu m = new PopupMenu(this, anchor);
        String[] names = {"Solid", "Dashed", "Dash-dot", "Dotted"};
        for (int i = 0; i < names.length; i++) {
            m.getMenu().add(0, i, i, names[i]).setCheckable(true).setChecked(tools.penStyle == i);
        }
        m.setOnMenuItemClickListener(i -> {
            tools.penStyle = i.getItemId();
            refreshTools();
            return true;
        });
        m.show();
    }

    private static String percent(int alpha) {
        return Math.round(alpha * 100f / 255f) + "%";
    }

    /** Opacity of the current tool: highlighter, or pen and shapes. */
    private void opacityDialog() {
        boolean hl = tools.tool == Tools.Tool.HIGHLIGHTER;
        int color = hl ? tools.hlColor : tools.penColor;
        alphaDialog(hl ? "Highlighter opacity" : "Pen opacity", hl ? tools.hlAlpha : tools.penAlpha, color, a -> {
            if (hl) tools.hlAlpha = a;
            else tools.penAlpha = a;
            refreshTools();
        });
    }

    private void fillOpacityDialog() {
        int color = tools.tool == Tools.Tool.HIGHLIGHTER ? tools.hlColor : tools.penColor;
        alphaDialog("Fill opacity", tools.fillAlpha, color, a -> {
            tools.fillAlpha = a;
            tools.fill = true;
            refreshTools();
        });
    }

    private interface AlphaCallback {
        void onAlpha(int alpha);
    }

    /** Slider from 5% to 100% with a preview over a checkerboard. */
    private void alphaDialog(String title, int alpha, int color, AlphaCallback cb) {
        LinearLayout box = new LinearLayout(this);
        box.setOrientation(LinearLayout.VERTICAL);
        int pad = Ui.dp(this, 20);
        box.setPadding(pad, pad / 2, pad, 0);
        View preview = new View(this) {
            private final Paint p = new Paint();

            @Override
            protected void onDraw(Canvas c) {
                int s = Ui.dp(getContext(), 8);
                for (int y = 0; y < getHeight(); y += s) {
                    for (int x = 0; x < getWidth(); x += s) {
                        p.setColor(((x / s + y / s) % 2 == 0) ? 0xffffffff : 0xffcccccc);
                        c.drawRect(x, y, x + s, y + s, p);
                    }
                }
                p.setColor(((int) getTag() << 24) | (color & 0xffffff));
                c.drawRect(0, 0, getWidth(), getHeight(), p);
            }
        };
        preview.setTag(alpha);
        box.addView(preview, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, Ui.dp(this, 48)));
        TextView label = Ui.label(this, percent(alpha), 15);
        label.setGravity(Gravity.CENTER);
        label.setPadding(0, Ui.dp(this, 10), 0, 0);
        box.addView(label);
        SeekBar bar = new SeekBar(this);
        bar.setMax(95);
        bar.setProgress(Math.round(alpha * 100f / 255f) - 5);
        bar.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override
            public void onProgressChanged(SeekBar sb, int progress, boolean user) {
                int a = Math.round((progress + 5) * 255f / 100f);
                preview.setTag(a);
                preview.invalidate();
                label.setText(percent(a));
            }

            @Override
            public void onStartTrackingTouch(SeekBar sb) {
            }

            @Override
            public void onStopTrackingTouch(SeekBar sb) {
            }
        });
        box.addView(bar);
        new AlertDialog.Builder(this).setTitle(title).setView(box)
                .setPositiveButton(android.R.string.ok, (d, w) -> {
                    cb.onAlpha((int) preview.getTag());
                    if (canvas.hasSelection()) Toast.makeText(this, "Applies to new strokes", Toast.LENGTH_SHORT).show();
                })
                .setNegativeButton(android.R.string.cancel, null).show();
    }

    private void textSizeDialog() {
        NumberPicker np = new NumberPicker(this);
        np.setMinValue(6);
        np.setMaxValue(96);
        np.setValue(Math.round(tools.textSize));
        new AlertDialog.Builder(this).setTitle("Text size (pt)").setView(np)
                .setPositiveButton(android.R.string.ok, (d, w) -> {
                    tools.textSize = np.getValue();
                    tools.save(prefs);
                })
                .setNegativeButton(android.R.string.cancel, null).show();
    }

    // ------------------------------------------------------------------ menu

    private static final int M_SAVE = 1, M_EXPORT = 2, M_SHARE = 3, M_ADD_BEFORE = 4, M_DUP_PAGE = 5,
            M_DEL_PAGE = 6, M_UP = 7, M_DOWN = 8, M_GOTO = 9, M_FIT = 10, M_PASTE = 11, M_STYLUS = 12,
            M_LAYERS = 13, M_ZOOM_IN = 14, M_ZOOM_OUT = 15, M_SCREEN = 16, M_SHARE_INK = 17;

    private void showMenu(View anchor) {
        if (doc == null) return;
        PopupMenu m = new PopupMenu(this, anchor);
        Menu menu = m.getMenu();
        boolean sketch = isSketch();
        if (!sketch) {
            menu.add(0, M_SAVE, 0, "Save");
            menu.add(0, M_EXPORT, 1, docUri != null ? "Export PDF to folder" : "Export as PDF…");
            menu.add(0, M_SHARE, 2, "Share as PDF");
            menu.add(0, M_SHARE_INK, 3, "Share note file (.ink)");
        }
        if (CanvasView.canPaste()) menu.add(0, M_PASTE, 4, "Paste");
        if (!sketch) {
            menu.add(0, M_ADD_BEFORE, 5, "Insert page before");
            menu.add(0, M_DUP_PAGE, 6, "Duplicate page");
            menu.add(0, M_UP, 7, "Move page up");
            menu.add(0, M_DOWN, 8, "Move page down");
            menu.add(0, M_DEL_PAGE, 9, "Delete page");
            menu.add(0, M_GOTO, 10, "Go to page…");
        }
        menu.add(0, M_LAYERS, 11, "Layers…");
        menu.add(0, M_ZOOM_IN, 12, "Zoom in");
        menu.add(0, M_ZOOM_OUT, 13, "Zoom out");
        menu.add(0, M_FIT, 14, "Fit width");
        menu.add(0, M_STYLUS, 15, "Draw with stylus only").setCheckable(true).setChecked(tools.stylusOnly);
        menu.add(0, M_SCREEN, 16, "Keep screen on").setCheckable(true).setChecked(prefs.getBoolean("keepScreenOn", false));
        m.setOnMenuItemClickListener(this::onMenu);
        m.show();
    }

    private boolean onMenu(MenuItem item) {
        int cur = canvas.currentPage();
        switch (item.getItemId()) {
            case M_SAVE:
                save(true, null);
                break;
            case M_EXPORT: {
                if (docUri != null) {
                    exportToFolder();
                    break;
                }
                Intent i = new Intent(Intent.ACTION_CREATE_DOCUMENT);
                i.addCategory(Intent.CATEGORY_OPENABLE);
                i.setType("application/pdf");
                i.putExtra(Intent.EXTRA_TITLE, Notes.title(note) + ".pdf");
                startActivityForResult(i, REQ_EXPORT);
                break;
            }
            case M_SHARE:
                exportTo(null);
                break;
            case M_SHARE_INK:
                save(false, () -> ShareProvider.share(this, note, "application/octet-stream"));
                break;
            case M_PASTE:
                canvas.paste();
                tools.tool = Tools.Tool.SELECT_RECT;
                refreshTools();
                break;
            case M_ADD_BEFORE:
                canvas.addPage(cur, doc.pages.get(cur).blankLike());
                canvas.scrollToPage(cur);
                break;
            case M_DUP_PAGE:
                canvas.addPage(cur + 1, doc.pages.get(cur).copy());
                canvas.scrollToPage(cur + 1);
                break;
            case M_UP:
                canvas.movePage(cur, -1);
                break;
            case M_DOWN:
                canvas.movePage(cur, 1);
                break;
            case M_DEL_PAGE:
                if (doc.pages.size() <= 1) {
                    Toast.makeText(this, "A note needs at least one page", Toast.LENGTH_SHORT).show();
                } else {
                    new AlertDialog.Builder(this).setMessage("Delete page " + (cur + 1) + "?")
                            .setPositiveButton("Delete", (d, w) -> canvas.deletePage(cur))
                            .setNegativeButton(android.R.string.cancel, null).show();
                }
                break;
            case M_GOTO:
                gotoDialog();
                break;
            case M_LAYERS:
                layersDialog();
                break;
            case M_ZOOM_IN:
                canvas.zoomBy(1.25f);
                break;
            case M_ZOOM_OUT:
                canvas.zoomBy(0.8f);
                break;
            case M_FIT:
                canvas.zoomFit();
                break;
            case M_STYLUS:
                tools.stylusOnly = !tools.stylusOnly;
                tools.save(prefs);
                Toast.makeText(this, tools.stylusOnly ? "Fingers scroll and zoom; the stylus draws"
                        : "Fingers draw; use two fingers to scroll", Toast.LENGTH_SHORT).show();
                break;
            case M_SCREEN: {
                boolean on = !prefs.getBoolean("keepScreenOn", false);
                prefs.edit().putBoolean("keepScreenOn", on).apply();
                if (on) getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
                else getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
                break;
            }
            default:
                return false;
        }
        return true;
    }

    private void addPageAfter() {
        if (doc == null) return;
        int cur = canvas.currentPage();
        canvas.addPage(cur + 1, doc.pages.get(cur).blankLike());
        canvas.scrollToPage(cur + 1);
    }

    private void gotoDialog() {
        if (doc == null) return;
        NumberPicker np = new NumberPicker(this);
        np.setMinValue(1);
        np.setMaxValue(doc.pages.size());
        np.setValue(canvas.currentPage() + 1);
        new AlertDialog.Builder(this).setTitle("Go to page").setView(np)
                .setPositiveButton(android.R.string.ok, (d, w) -> canvas.scrollToPage(np.getValue() - 1))
                .setNegativeButton(android.R.string.cancel, null).show();
    }

    private void layersDialog() {
        Doc.Page page = doc.pages.get(canvas.currentPage());
        LinearLayout box = new LinearLayout(this);
        box.setOrientation(LinearLayout.VERTICAL);
        int pad = Ui.dp(this, 16);
        box.setPadding(pad, pad / 2, pad, 0);
        java.util.List<android.widget.RadioButton> radios = new java.util.ArrayList<>();
        for (int i = page.layers.size() - 1; i >= 0; i--) {
            Doc.Layer l = page.layers.get(i);
            final int index = i;
            LinearLayout r = new LinearLayout(this);
            r.setGravity(Gravity.CENTER_VERTICAL);
            android.widget.RadioButton rb = new android.widget.RadioButton(this);
            rb.setText(l.name != null ? l.name : "Layer " + (i + 1));
            rb.setChecked(i == page.activeLayer);
            radios.add(rb);
            rb.setOnClickListener(v -> {
                for (android.widget.RadioButton o : radios) o.setChecked(o == rb);
                page.activeLayer = index;
            });
            CheckBox vis = new CheckBox(this);
            vis.setText("Visible");
            vis.setChecked(l.visible);
            vis.setOnCheckedChangeListener((b, on) -> {
                l.visible = on;
                dirty = true;
                canvas.invalidate();
            });
            r.addView(rb, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1));
            r.addView(vis);
            box.addView(r);
        }
        new AlertDialog.Builder(this).setTitle("Layers of page " + (canvas.currentPage() + 1)).setView(box)
                .setPositiveButton("Done", null)
                .setNeutralButton("New layer", (d, w) -> {
                    Doc.Layer l = new Doc.Layer();
                    l.name = "Layer " + (page.layers.size() + 1);
                    page.layers.add(l);
                    page.activeLayer = page.layers.size() - 1;
                    dirty = true;
                    Toast.makeText(this, "Drawing on " + l.name, Toast.LENGTH_SHORT).show();
                }).show();
    }

    private void renameDialog() {
        EditText e = new EditText(this);
        e.setSingleLine(true);
        e.setText(Notes.title(note));
        e.setSelectAllOnFocus(true);
        new AlertDialog.Builder(this).setTitle("Rename note").setView(e)
                .setPositiveButton(android.R.string.ok, (d, w) -> save(false, () -> {
                    try {
                        if (docUri != null) renameInFolder(Notes.sanitize(e.getText().toString()) + InkFile.EXT);
                        note = Notes.rename(note, e.getText().toString());
                        if (doc.pdf != null && doc.pdf.getName().endsWith(InkFile.BG_SUFFIX)) {
                            doc.pdf = InkFile.bgPdfFor(note);
                        }
                        titleView.setText(Notes.title(note));
                        getIntent().putExtra(EXTRA_PATH, note.getPath());
                    } catch (IOException ex) {
                        Toast.makeText(this, ex.getMessage(), Toast.LENGTH_LONG).show();
                    }
                }))
                .setNegativeButton(android.R.string.cancel, null).show();
    }

    // ------------------------------------------------------------------ host callbacks

    @Override
    public void onTextTap(Doc.Page page, float x, float y, TextEl existing) {
        LinearLayout box = new LinearLayout(this);
        box.setOrientation(LinearLayout.VERTICAL);
        int pad = Ui.dp(this, 16);
        box.setPadding(pad, pad / 2, pad, 0);
        EditText e = new EditText(this);
        e.setInputType(InputType.TYPE_CLASS_TEXT | InputType.TYPE_TEXT_FLAG_MULTI_LINE | InputType.TYPE_TEXT_FLAG_CAP_SENTENCES);
        e.setMinLines(3);
        e.setGravity(Gravity.TOP);
        if (existing != null) e.setText(existing.text);
        box.addView(e);
        TextView sizeLabel = Ui.label(this, "", 13);
        box.addView(sizeLabel);
        SeekBar size = new SeekBar(this);
        size.setMax(90);
        float initial = existing != null ? existing.size : tools.textSize;
        size.setProgress(Math.round(initial) - 6);
        sizeLabel.setText("Size: " + Math.round(initial) + " pt");
        size.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override
            public void onProgressChanged(SeekBar s, int p, boolean u) {
                sizeLabel.setText("Size: " + (p + 6) + " pt");
            }

            @Override
            public void onStartTrackingTouch(SeekBar s) {
            }

            @Override
            public void onStopTrackingTouch(SeekBar s) {
            }
        });
        box.addView(size);
        AlertDialog.Builder b = new AlertDialog.Builder(this)
                .setTitle(existing == null ? "Add text" : "Edit text")
                .setView(box)
                .setPositiveButton(android.R.string.ok, (d, w) -> {
                    String s = e.getText().toString();
                    if (s.trim().isEmpty()) {
                        if (existing != null) canvas.putText(page, existing, null);
                        return;
                    }
                    TextEl t = existing != null ? (TextEl) existing.copy() : new TextEl();
                    if (existing == null) {
                        t.x = x;
                        t.y = y;
                        t.color = tools.textColor;
                    }
                    t.text = s;
                    t.size = size.getProgress() + 6;
                    t.invalidate();
                    tools.textSize = t.size;
                    tools.save(prefs);
                    canvas.putText(page, existing, t);
                })
                .setNegativeButton(android.R.string.cancel, null);
        if (existing != null) b.setNeutralButton("Delete", (d, w) -> canvas.putText(page, existing, null));
        AlertDialog dlg = b.create();
        dlg.getWindow().setSoftInputMode(WindowManager.LayoutParams.SOFT_INPUT_STATE_VISIBLE);
        dlg.show();
        e.requestFocus();
    }

    @Override
    public void onSelectionChanged(boolean has) {
        selectionBar.setVisibility(has ? View.VISIBLE : View.GONE);
    }

    private final Runnable autosave = () -> {
        if (dirty && !saving) save(false, null);
    };

    @Override
    public void onEdited() {
        dirty = true;
        updateUndo();
        main.removeCallbacks(autosave);
        main.postDelayed(autosave, 3000);
    }

    @Override
    public void onStylusSeen() {
        if (!tools.stylusOnly && !prefs.getBoolean("stylusAsked", false)) {
            prefs.edit().putBoolean("stylusAsked", true).apply();
            tools.stylusOnly = true;
            tools.save(prefs);
            Toast.makeText(this, "Stylus detected: the pen draws, fingers scroll and zoom. Change this in the menu.",
                    Toast.LENGTH_LONG).show();
        }
    }

    @Override
    public void onPageChanged(int index, int total) {
        pageView.setText((index + 1) + " / " + total);
    }

    private void afterUndo() {
        canvas.afterUndoRedo();
        onEdited();
    }

    private void updateUndo() {
        boolean u = undo != null && undo.canUndo(), r = undo != null && undo.canRedo();
        undoBtn.setEnabled(u);
        undoBtn.setAlpha(u ? 1f : 0.35f);
        redoBtn.setEnabled(r);
        redoBtn.setAlpha(r ? 1f : 0.35f);
    }

    // ------------------------------------------------------------------ images

    private void pickImage() {
        Intent i = new Intent(Intent.ACTION_GET_CONTENT);
        i.setType("image/*");
        i.addCategory(Intent.CATEGORY_OPENABLE);
        startActivityForResult(Intent.createChooser(i, "Insert image"), REQ_IMAGE);
    }

    private Bitmap decodeImage(Uri uri) throws IOException {
        BitmapFactory.Options o = new BitmapFactory.Options();
        o.inJustDecodeBounds = true;
        try (InputStream in = getContentResolver().openInputStream(uri)) {
            BitmapFactory.decodeStream(in, null, o);
        }
        int sample = 1;
        while (Math.max(o.outWidth, o.outHeight) / sample > 2400) sample *= 2;
        o = new BitmapFactory.Options();
        o.inSampleSize = sample;
        try (InputStream in = getContentResolver().openInputStream(uri)) {
            Bitmap b = BitmapFactory.decodeStream(in, null, o);
            if (b == null) throw new IOException("Unsupported image");
            return b;
        }
    }

    @Override
    protected void onActivityResult(int req, int res, Intent data) {
        super.onActivityResult(req, res, data);
        if (res != RESULT_OK || data == null || data.getData() == null || doc == null) return;
        Uri uri = data.getData();
        if (req == REQ_IMAGE) {
            try {
                canvas.insertImage(decodeImage(uri));
                tools.tool = Tools.Tool.SELECT_RECT;
                refreshTools();
            } catch (IOException | RuntimeException e) {
                Toast.makeText(this, "Could not load the image: " + e.getMessage(), Toast.LENGTH_LONG).show();
            }
        } else if (req == REQ_EXPORT) {
            exportTo(uri);
        } else if (req == REQ_LOCATE_PDF) {
            File bg = InkFile.bgPdfFor(note);
            try {
                Notes.copy(getContentResolver(), uri, bg);
                if (docUri != null) linkBackground(uri, bg);
                doc.pdf = bg;
                doc.pdfMissing = false;
                openPdf();
                dirty = true;
                canvas.invalidate();
            } catch (IOException e) {
                Toast.makeText(this, "Could not copy the PDF: " + e.getMessage(), Toast.LENGTH_LONG).show();
            }
        }
    }

    // ------------------------------------------------------------------ saving and export

    private byte[] preview() {
        try {
            Bitmap b = canvas.renderPage(0, 360);
            ByteArrayOutputStream out = new ByteArrayOutputStream();
            b.compress(Bitmap.CompressFormat.PNG, 100, out);
            return out.toByteArray();
        } catch (RuntimeException | OutOfMemoryError e) {
            return null;
        }
    }

    /** Saves if needed, then runs {@code then} on the UI thread. */
    private void save(boolean announce, Runnable then) {
        if (doc == null) {
            if (then != null) then.run();
            return;
        }
        if (!dirty && !announce) {
            if (then != null) then.run();
            return;
        }
        Doc snap = doc.snapshot();
        byte[] png = preview();
        File target = note;
        dirty = false;
        saving = true;
        io.execute(() -> {
            try {
                InkFile.write(snap, target, png);
                if (docUri != null) SafDocs.copy(getContentResolver(), target, docUri);
                main.post(() -> {
                    saving = false;
                    if (announce) Toast.makeText(this, "Saved", Toast.LENGTH_SHORT).show();
                    if (then != null) then.run();
                });
            } catch (IOException | RuntimeException e) {
                main.post(() -> {
                    saving = false;
                    dirty = true;
                    Toast.makeText(this, "Save failed: " + e.getMessage(), Toast.LENGTH_LONG).show();
                });
            }
        });
    }

    /**
     * Writes "&lt;note&gt;.pdf" (or "&lt;note&gt; (annotated).pdf" for a PDF annotation) next to the
     * note in the library folder, asking before replacing a file.
     */
    private void exportToFolder() {
        String title = Notes.title(note);
        String name = title + (doc.usesPdf() ? " (annotated)" : "") + ".pdf";
        SafDocs.Entry existing = SafDocs.child(getContentResolver(), treeUri, parentId, name);
        Runnable go = () -> exportTo(null, existing != null ? existing.uri : null, name);
        if (existing == null) go.run();
        else new AlertDialog.Builder(this).setMessage("Replace “" + name + "” in the folder?")
                .setPositiveButton("Replace", (d, w) -> go.run())
                .setNegativeButton(android.R.string.cancel, null).show();
    }

    private void exportTo(Uri uri) {
        exportTo(uri, null, null);
    }

    /** Exports a vector PDF to {@code uri}, or shares it when uri is null. */
    @SuppressWarnings("deprecation")
    private void exportTo(Uri uri, Uri folderTarget, String folderName) {
        Doc snap = doc.snapshot();
        ProgressDialog pd = new ProgressDialog(this);
        pd.setMessage("Exporting PDF…");
        pd.setProgressStyle(ProgressDialog.STYLE_HORIZONTAL);
        pd.setMax(snap.pages.size());
        pd.setCancelable(false);
        pd.show();
        io.execute(() -> {
            File dir = new File(getCacheDir(), "share");
            dir.mkdirs();
            File out = new File(dir, Notes.sanitize(Notes.title(note)) + ".pdf");
            try {
                PdfExport.export(snap, out, (done, total) -> main.post(() -> pd.setProgress(done)));
                if (uri != null) Notes.copy(getContentResolver(), out, uri);
                if (folderName != null) {
                    Uri target = folderTarget != null ? folderTarget
                            : SafDocs.create(getContentResolver(), treeUri, parentId, "application/pdf", folderName);
                    SafDocs.copy(getContentResolver(), out, target);
                }
                main.post(() -> {
                    pd.dismiss();
                    if (folderName != null) Toast.makeText(this, "Saved “" + folderName + "” in the folder", Toast.LENGTH_LONG).show();
                    else if (uri == null) ShareProvider.share(this, out, "application/pdf");
                    else Toast.makeText(this, "PDF exported", Toast.LENGTH_SHORT).show();
                });
            } catch (IOException | RuntimeException | OutOfMemoryError e) {
                main.post(() -> {
                    pd.dismiss();
                    Toast.makeText(this, "Export failed: " + e.getMessage(), Toast.LENGTH_LONG).show();
                });
            }
        });
    }

    @Override
    protected void onPause() {
        super.onPause();
        tools.save(prefs);
        if (dirty) save(false, null);
    }

    @Override
    public void onBackPressed() {
        if (canvas != null && canvas.hasSelection()) {
            canvas.clearSelection();
            return;
        }
        if (isSketch()) {
            if (undo == null || !undo.canUndo()) {
                setResult(RESULT_CANCELED);
                finish();
                return;
            }
            new AlertDialog.Builder(this).setMessage("Discard this sketch?")
                    .setPositiveButton("Discard", (d, w) -> {
                        setResult(RESULT_CANCELED);
                        finish();
                    })
                    .setNeutralButton("Insert", (d, w) -> finishSketch())
                    .setNegativeButton(android.R.string.cancel, null).show();
            return;
        }
        save(false, () -> {
            setResult(RESULT_OK);
            finish();
        });
    }

    // ------------------------------------------------------------------ library folder (SAF)

    /** Copies the note and its attached PDF from the library folder into the working copy. */
    private void pullFromFolder() throws IOException {
        SafDocs.copy(getContentResolver(), docUri, note);
        File bg = InkFile.bgPdfFor(note);
        bg.delete();
        String[] link = InkFile.readPdfLink(new java.io.FileInputStream(note));
        if (link != null && (link[0] != null || link[2] != null)) {
            // Linked PDF, rendered from a local copy; the PDF itself is never changed.
            // Try the saved link, then the saved path, then the same name in the folder.
            boolean ok = false;
            if (link[0] != null) {
                try {
                    SafDocs.copy(getContentResolver(), Uri.parse(link[0]), bg);
                    ok = true;
                } catch (IOException | RuntimeException e) {
                    bg.delete();
                }
            }
            File byPath = ok ? null : PdfLinks.readable(link[2]);
            if (byPath != null) {
                Notes.copy(byPath, bg);
                ok = true;
            }
            if (!ok && PdfLinks.hasFileAccess(this)) {
                // With file access: the old link's real file, else a file with the same name.
                File f = link[0] == null ? null : PdfLinks.readable(PdfLinks.realPath(this, Uri.parse(link[0])));
                if (f == null) f = PdfLinks.findOnDevice(link[1]);
                if (f != null) {
                    Notes.copy(f, bg);
                    foundPath = f.getPath();
                    ok = true;
                }
            }
            if (!ok) {
                SafDocs.Entry found = SafDocs.child(getContentResolver(), treeUri, parentId, link[1]);
                if (found == null) found = PdfLinks.findByName(getContentResolver(), treeUri, SafDocs.rootId(treeUri), link[1], 5);
                if (found != null) {
                    SafDocs.copy(getContentResolver(), found.uri, bg);
                    foundPdf = found.uri;
                }
            }
        } else {
            // Older notes keep an attached copy next to them.
            SafDocs.Entry sibling = SafDocs.child(getContentResolver(), treeUri, parentId, note.getName() + InkFile.BG_SUFFIX);
            if (sibling != null) SafDocs.copy(getContentResolver(), sibling.uri, bg);
        }
        Recents.add(this, docUri.toString(), note.getName());
    }

    /** Links the chosen PDF to the note; falls back to an attached copy if the grant cannot be kept. */
    private void linkBackground(Uri pdf, File bg) throws IOException {
        String oldUri = doc.pdfUri, oldPath = doc.pdfPath;
        String oldName = doc.pdfName == null ? null : new File(doc.pdfName).getName();
        try {
            getContentResolver().takePersistableUriPermission(pdf, Intent.FLAG_GRANT_READ_URI_PERMISSION);
            doc.pdfUri = pdf.toString();
            doc.pdfName = Notes.displayName(this, pdf, "document.pdf");
            doc.pdfPath = PdfLinks.realPath(this, pdf);
            relinkOthers(oldUri, oldPath, oldName, doc.pdfUri, doc.pdfPath, doc.pdfName);
        } catch (SecurityException e) {
            doc.pdfUri = null;
            pushBackground(bg);
        }
    }

    /**
     * Other notes in this folder that pointed at the same missing PDF get the new link too.
     * Runs in the background; reports how many were updated.
     */
    private void relinkOthers(String oldUri, String oldPath, String oldName, String newUri, String newPath, String newName) {
        if (oldUri == null && oldPath == null && oldName == null) return;
        String self = docUri.toString();
        io.execute(() -> {
            int n = 0;
            File dir = new File(getCacheDir(), "share/relink");
            dir.mkdirs();
            for (SafDocs.Entry e : SafDocs.list(getContentResolver(), treeUri, parentId)) {
                if (!e.name.endsWith(InkFile.EXT) || e.uri.toString().equals(self)) continue;
                try {
                    String[] l = InkFile.readPdfLink(getContentResolver().openInputStream(e.uri));
                    if (l == null) continue;
                    boolean same = (oldUri != null && oldUri.equals(l[0])) || (oldPath != null && oldPath.equals(l[2]))
                            || (oldUri == null && oldPath == null && l[0] == null && oldName != null && oldName.equals(l[1]));
                    if (!same) continue;
                    File tmp = new File(dir, e.name);
                    SafDocs.copy(getContentResolver(), e.uri, tmp);
                    Doc d = InkFile.read(tmp);
                    d.pdfUri = newUri;
                    d.pdfPath = newPath;
                    d.pdfName = newName;
                    InkFile.write(d, tmp, null);
                    SafDocs.copy(getContentResolver(), tmp, e.uri);
                    tmp.delete();
                    n++;
                } catch (IOException | RuntimeException ignored) {
                    // leave that note as it was
                }
            }
            int count = n;
            if (count > 0) main.post(() -> Toast.makeText(this, "Also relinked " + count + " other note"
                    + (count == 1 ? "" : "s") + " to this PDF", Toast.LENGTH_SHORT).show());
        });
    }

    /** Writes a background PDF next to the note in the library folder. */
    private void pushBackground(File bg) throws IOException {
        String name = note.getName() + InkFile.BG_SUFFIX;
        SafDocs.Entry sibling = SafDocs.child(getContentResolver(), treeUri, parentId, name);
        Uri target = sibling != null ? sibling.uri : SafDocs.create(getContentResolver(), treeUri, parentId, "application/pdf", name);
        SafDocs.copy(getContentResolver(), bg, target);
    }

    private void renameInFolder(String newName) throws IOException {
        if (SafDocs.child(getContentResolver(), treeUri, parentId, newName) != null) {
            throw new IOException("A file with that name already exists");
        }
        SafDocs.Entry sibling = SafDocs.child(getContentResolver(), treeUri, parentId, note.getName() + InkFile.BG_SUFFIX);
        Uri renamed = SafDocs.rename(getContentResolver(), docUri, newName);
        if (sibling != null) SafDocs.rename(getContentResolver(), sibling.uri, newName + InkFile.BG_SUFFIX);
        Recents.rename(this, docUri.toString(), renamed.toString(), newName);
        docUri = renamed;
        getIntent().putExtra(EXTRA_DOC_URI, renamed.toString()).putExtra(EXTRA_NAME, newName);
    }

    // ------------------------------------------------------------------ sketch mode (Markd)

    /** True in {@link SketchActivity}: one canvas whose result goes back to the caller as SVG. */
    protected boolean isSketch() {
        return false;
    }

    /** Working file for a sketch: the note embedded in the caller's SVG, or a blank page. */
    private File prepareSketch() {
        File dir = new File(getCacheDir(), "sketch");
        dir.mkdirs();
        File f = new File(dir, "sketch" + InkFile.EXT);
        InkFile.bgPdfFor(f).delete();
        try {
            byte[] ink = null;
            String b64 = getIntent().getStringExtra(SketchActivity.EXTRA_INK);
            if (b64 != null) ink = android.util.Base64.decode(b64, android.util.Base64.DEFAULT);
            if (ink == null) ink = SvgSketch.embeddedNote(getIntent().getStringExtra(SketchActivity.EXTRA_SVG));
            if (ink != null) {
                try (java.io.OutputStream out = new java.io.FileOutputStream(f)) {
                    out.write(ink);
                }
                InkFile.read(f);
            } else {
                InkFile.write(Doc.blank(), f, null);
            }
            return f;
        } catch (IOException | IllegalArgumentException e) {
            Toast.makeText(this, "Could not open the sketch: " + e.getMessage(), Toast.LENGTH_LONG).show();
            try {
                InkFile.write(Doc.blank(), f, null);
                return f;
            } catch (IOException e2) {
                return null;
            }
        }
    }

    private void finishSketch() {
        if (doc == null) return;
        canvas.clearSelection();
        Doc snap = doc.snapshot();
        if (SvgSketch.contentBounds(snap) == null) {
            Toast.makeText(this, "Draw something first", Toast.LENGTH_SHORT).show();
            return;
        }
        boolean paper = getIntent().getBooleanExtra(SketchActivity.EXTRA_PAPER, true);
        io.execute(() -> {
            try {
                String svg = SvgSketch.write(snap, paper);
                File dir = new File(getCacheDir(), "share");
                dir.mkdirs();
                File out = new File(dir, "sketch-" + System.currentTimeMillis() + ".svg");
                try (java.io.Writer w = new java.io.OutputStreamWriter(new java.io.FileOutputStream(out),
                        java.nio.charset.StandardCharsets.UTF_8)) {
                    w.write(svg);
                }
                Uri uri = ShareProvider.uriFor(this, out);
                main.post(() -> {
                    Intent res = new Intent();
                    res.setDataAndType(uri, "image/svg+xml");
                    res.setClipData(android.content.ClipData.newRawUri("sketch", uri));
                    res.addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
                    setResult(RESULT_OK, res);
                    finish();
                });
            } catch (IOException | RuntimeException e) {
                main.post(() -> Toast.makeText(this, "Could not create the sketch: " + e.getMessage(), Toast.LENGTH_LONG).show());
            }
        });
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        main.removeCallbacks(autosave);
        if (pdf != null) pdf.close();
        io.shutdown();
    }
}
