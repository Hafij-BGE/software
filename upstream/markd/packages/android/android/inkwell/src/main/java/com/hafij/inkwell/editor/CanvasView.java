package com.hafij.inkwell.editor;

import android.content.Context;
import android.content.res.Configuration;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.DashPathEffect;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.RectF;
import android.util.AttributeSet;
import android.view.MotionEvent;
import android.view.VelocityTracker;
import android.view.View;
import android.view.ViewConfiguration;
import android.widget.OverScroller;

import com.hafij.inkwell.model.Doc;
import com.hafij.inkwell.model.Element;
import com.hafij.inkwell.model.ImageEl;
import com.hafij.inkwell.model.Stroke;
import com.hafij.inkwell.model.Template;
import com.hafij.inkwell.model.TextEl;
import com.hafij.inkwell.pdf.PdfPages;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

/** Continuous vertical page view with drawing, erasing, shapes, selection and pinch zoom. */
public class CanvasView extends View implements PdfPages.Listener {
    public interface Host {
        void onTextTap(Doc.Page page, float x, float y, TextEl existing);

        void onSelectionChanged(boolean hasSelection);

        void onEdited();

        void onStylusSeen();

        void onPageChanged(int index, int total);
    }

    /** Elements picked with a selection tool, all on one layer. */
    public static final class Selection {
        public final Doc.Page page;
        public final Doc.Layer layer;
        public List<Element> items;

        Selection(Doc.Page page, Doc.Layer layer, List<Element> items) {
            this.page = page;
            this.layer = layer;
            this.items = items;
        }

        public RectF bounds() {
            RectF r = null;
            for (Element e : items) {
                if (r == null) r = new RectF(e.bounds());
                else r.union(e.bounds());
            }
            return r == null ? new RectF() : r;
        }
    }

    private static final float GAP = 14f;
    private static final List<Element> CLIPBOARD = new ArrayList<>();

    private Doc doc;
    private Undo undo;
    private Tools tools;
    private PdfPages pdf;
    private Host host;

    private float zoom = 1f, fitZoom = 1f;
    private float offX, offY;
    private float[] tops = new float[0];
    private float docW, docH;
    private boolean needsFit = true;
    private int lastPage = -1;
    private final float density;
    private final float touchSlop;

    // drawing gesture
    private enum Mode {NONE, DRAW, NAV}

    private Mode mode = Mode.NONE;
    private int drawPointer = -1;
    private boolean drawByStylus;
    private Tools.Tool drawTool;
    private Doc.Page drawPage;
    private Stroke live;
    private float[] pts = new float[256];
    private float[] prs = new float[128];
    private int npts;
    private boolean usePressure;
    private float startX, startY, lastX, lastY, downSX, downSY;
    private boolean moved;
    private boolean erased;

    // selection
    private Selection sel;
    private int selDrag; // 0 none, 1 move, 2 scale
    private float[] lasso = new float[256];
    private int nlasso;

    // navigation
    private float focusX, focusY, span;
    private VelocityTracker velocity;
    private final OverScroller scroller;
    private boolean pinching;

    private final Paint deskPaint = new Paint();
    private final Paint shadowPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint bgPaint = new Paint();
    private final Paint bmpPaint = new Paint(Paint.FILTER_BITMAP_FLAG);
    private final Paint linePaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint selPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint handlePaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint cursorPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF tmpRect = new RectF();
    private float cursorX = Float.NaN, cursorY;

    public CanvasView(Context c) {
        this(c, null);
    }

    public CanvasView(Context c, AttributeSet a) {
        super(c, a);
        density = getResources().getDisplayMetrics().density;
        touchSlop = ViewConfiguration.get(c).getScaledTouchSlop();
        scroller = new OverScroller(c);
        boolean night = (getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK)
                == Configuration.UI_MODE_NIGHT_YES;
        deskPaint.setColor(night ? 0xff1e1f22 : 0xffe4e6ea);
        shadowPaint.setColor(night ? 0x66000000 : 0x33000000);
        linePaint.setStyle(Paint.Style.STROKE);
        selPaint.setStyle(Paint.Style.STROKE);
        selPaint.setColor(0xff1a73e8);
        handlePaint.setColor(0xff1a73e8);
        cursorPaint.setStyle(Paint.Style.STROKE);
        cursorPaint.setColor(0x99000000);
        setFocusable(true);
    }

    public void setup(Doc doc, Undo undo, Tools tools, PdfPages pdf, Host host) {
        this.doc = doc;
        this.undo = undo;
        this.tools = tools;
        this.pdf = pdf;
        this.host = host;
        needsFit = true;
        relayout();
        if (getWidth() > 0) onSizeChanged(getWidth(), getHeight(), 0, 0);
        invalidate();
    }

    // ------------------------------------------------------------------ layout

    public void relayout() {
        if (doc == null) return;
        int n = doc.pages.size();
        tops = new float[n];
        float y = GAP, w = 0;
        for (int i = 0; i < n; i++) {
            Doc.Page p = doc.pages.get(i);
            tops[i] = y;
            y += p.height + GAP;
            w = Math.max(w, p.width);
        }
        docW = w + 2 * GAP;
        docH = y;
        if (sel != null && !doc.pages.contains(sel.page)) clearSelection();
        clamp();
        invalidate();
    }

    private float pageLeft(Doc.Page p) {
        return (docW - p.width) / 2;
    }

    @Override
    protected void onSizeChanged(int w, int h, int ow, int oh) {
        super.onSizeChanged(w, h, ow, oh);
        if (doc == null) return;
        float oldFit = fitZoom;
        fitZoom = docW > 0 ? w / docW : 1;
        if (needsFit || ow == 0) {
            zoom = fitZoom;
            offY = 0;
            needsFit = false;
        } else if (oldFit > 0) {
            // Keep the same relative zoom across rotation.
            float centerDocY = (oh / 2f - offY) / zoom;
            zoom = zoom / oldFit * fitZoom;
            offY = h / 2f - centerDocY * zoom;
        }
        clamp();
    }

    private void clamp() {
        int w = getWidth(), h = getHeight();
        if (w == 0) return;
        float cw = docW * zoom, ch = docH * zoom;
        offX = cw <= w ? (w - cw) / 2 : Math.max(w - cw, Math.min(0, offX));
        offY = ch <= h ? Math.min(0, Math.max(offY, 0)) : Math.max(h - ch, Math.min(0, offY));
    }

    public int currentPage() {
        if (doc == null || tops.length == 0) return 0;
        float y = (getHeight() / 2f - offY) / zoom;
        int i = Arrays.binarySearch(tops, y);
        if (i < 0) i = -i - 2;
        return Math.max(0, Math.min(tops.length - 1, i));
    }

    public void scrollToPage(int i) {
        if (i < 0 || i >= tops.length) return;
        scroller.forceFinished(true);
        offY = -(tops[i] - GAP / 2) * zoom;
        clamp();
        invalidate();
    }

    public void zoomBy(float f) {
        zoomAround(getWidth() / 2f, getHeight() / 2f, getWidth() / 2f, getHeight() / 2f, f);
        invalidate();
    }

    public void zoomFit() {
        float y = (getHeight() / 2f - offY) / zoom;
        zoom = fitZoom;
        offY = getHeight() / 2f - y * zoom;
        clamp();
        invalidate();
    }

    private void zoomAround(float fx, float fy, float nfx, float nfy, float factor) {
        float dx = (fx - offX) / zoom, dy = (fy - offY) / zoom;
        zoom = Math.max(fitZoom * 0.3f, Math.min(fitZoom * 10f, zoom * factor));
        offX = nfx - dx * zoom;
        offY = nfy - dy * zoom;
        clamp();
    }

    // ------------------------------------------------------------------ drawing

    @Override
    public void onRendered(int page) {
        invalidate();
    }

    @Override
    protected void onDraw(Canvas c) {
        c.drawPaint(deskPaint);
        if (doc == null) return;
        c.save();
        c.translate(offX, offY);
        c.scale(zoom, zoom);
        float top = -offY / zoom, bottom = (getHeight() - offY) / zoom;
        for (int i = 0; i < doc.pages.size() && i < tops.length; i++) {
            Doc.Page p = doc.pages.get(i);
            float t = tops[i];
            if (t + p.height < top || t > bottom) continue;
            float l = pageLeft(p);
            c.save();
            c.translate(l, t);
            float sh = 1.5f / zoom * density;
            c.drawRect(-sh / 2, -sh / 2, p.width + sh, p.height + sh, shadowPaint);
            drawBackground(c, p);
            c.clipRect(0, 0, p.width, p.height);
            for (Doc.Layer layer : p.layers) {
                if (!layer.visible) continue;
                for (Element e : layer.elements) e.draw(c);
            }
            if (live != null && drawPage == p) live.draw(c);
            c.restore();
        }
        drawSelectionOverlay(c);
        c.restore();
        int cur = currentPage();
        if (cur != lastPage) {
            lastPage = cur;
            if (host != null) host.onPageChanged(cur, doc.pages.size());
        }
    }

    private void drawBackground(Canvas c, Doc.Page p) {
        Doc.Background bg = p.bg;
        bgPaint.setColor(bg.type == Doc.Background.Type.SOLID ? bg.color | 0xff000000 : 0xffffffff);
        c.drawRect(0, 0, p.width, p.height, bgPaint);
        if (bg.type == Doc.Background.Type.PDF && pdf != null && !doc.pdfMissing) {
            PdfPages.Rendered r = pdf.cached(bg.pdfPage);
            if (r != null) {
                tmpRect.set(0, 0, p.width, p.height);
                c.drawBitmap(r.bitmap, null, tmpRect, bmpPaint);
            }
            if (!pinching) pdf.request(bg.pdfPage, zoom, p.width, p.height);
        } else if (bg.type == Doc.Background.Type.SOLID && !"plain".equals(bg.style)) {
            for (Template.Group g : Template.geometry(bg.style, bg.config, p.width, p.height)) {
                linePaint.setColor(g.color);
                linePaint.setStrokeWidth(g.width);
                linePaint.setStrokeCap(g.dots ? Paint.Cap.ROUND : Paint.Cap.BUTT);
                if (g.dots) c.drawPoints(g.coords, linePaint);
                else c.drawLines(g.coords, linePaint);
            }
        }
    }

    private void drawSelectionOverlay(Canvas c) {
        float px = 1f / zoom;
        if (mode == Mode.DRAW && (drawTool == Tools.Tool.SELECT_LASSO || drawTool == Tools.Tool.SELECT_RECT)
                && selDrag == 0 && drawPage != null && nlasso >= 2) {
            c.save();
            c.translate(pageLeft(drawPage), topOf(drawPage));
            selPaint.setStrokeWidth(1.5f * px * density);
            selPaint.setPathEffect(new DashPathEffect(new float[]{6 * px * density, 4 * px * density}, 0));
            if (drawTool == Tools.Tool.SELECT_RECT) {
                c.drawRect(Math.min(lasso[0], lasso[2]), Math.min(lasso[1], lasso[3]),
                        Math.max(lasso[0], lasso[2]), Math.max(lasso[1], lasso[3]), selPaint);
            } else {
                Path path = new Path();
                path.moveTo(lasso[0], lasso[1]);
                for (int i = 1; i < nlasso; i++) path.lineTo(lasso[2 * i], lasso[2 * i + 1]);
                path.close();
                c.drawPath(path, selPaint);
            }
            selPaint.setPathEffect(null);
            c.restore();
        }
        if (sel != null && doc.pages.contains(sel.page)) {
            c.save();
            c.translate(pageLeft(sel.page), topOf(sel.page));
            RectF b = sel.bounds();
            float pad = 4 * px * density;
            b.inset(-pad, -pad);
            selPaint.setStrokeWidth(1.5f * px * density);
            selPaint.setPathEffect(new DashPathEffect(new float[]{6 * px * density, 4 * px * density}, 0));
            c.drawRect(b, selPaint);
            selPaint.setPathEffect(null);
            c.drawCircle(b.right, b.bottom, 7 * px * density, handlePaint);
            c.restore();
        }
        if (!Float.isNaN(cursorX) && drawPage != null && mode == Mode.DRAW && drawTool == Tools.Tool.ERASER) {
            cursorPaint.setStrokeWidth(px * density);
            c.drawCircle(pageLeft(drawPage) + cursorX, topOf(drawPage) + cursorY, tools.eraserWidth() / 2, cursorPaint);
        }
    }

    private float topOf(Doc.Page p) {
        int i = doc.pages.indexOf(p);
        return i >= 0 && i < tops.length ? tops[i] : 0;
    }

    // ------------------------------------------------------------------ input

    private Doc.Page pageAtScreen(float sx, float sy, float[] out) {
        float dx = (sx - offX) / zoom, dy = (sy - offY) / zoom;
        int n = Math.min(doc.pages.size(), tops.length);
        if (n == 0) return null;
        int best = 0;
        for (int i = 0; i < n; i++) {
            if (dy >= tops[i] - GAP / 2) best = i;
        }
        Doc.Page p = doc.pages.get(best);
        out[0] = dx - pageLeft(p);
        out[1] = dy - tops[best];
        return p;
    }

    private void toPage(Doc.Page p, float sx, float sy, float[] out) {
        out[0] = (sx - offX) / zoom - pageLeft(p);
        out[1] = (sy - offY) / zoom - topOf(p);
    }

    @Override
    public boolean onTouchEvent(MotionEvent e) {
        if (doc == null) return false;
        int action = e.getActionMasked();
        int idx = e.getActionIndex();
        int type = e.getToolType(idx);
        boolean stylus = type == MotionEvent.TOOL_TYPE_STYLUS || type == MotionEvent.TOOL_TYPE_ERASER;
        if (stylus && action == MotionEvent.ACTION_DOWN && host != null) host.onStylusSeen();

        if (velocity == null) velocity = VelocityTracker.obtain();
        velocity.addMovement(e);

        switch (action) {
            case MotionEvent.ACTION_DOWN: {
                scroller.forceFinished(true);
                boolean nav = tools.tool == Tools.Tool.HAND || (!stylus && tools.stylusOnly);
                if (nav) startNav(e);
                else startDraw(e, idx, stylus);
                break;
            }
            case MotionEvent.ACTION_POINTER_DOWN:
                if (mode == Mode.DRAW && !drawByStylus) {
                    cancelDraw();
                    startNav(e);
                } else if (mode == Mode.NAV) {
                    navRef(e, -1);
                }
                break;
            case MotionEvent.ACTION_MOVE:
                if (mode == Mode.DRAW) moveDraw(e);
                else if (mode == Mode.NAV) moveNav(e);
                break;
            case MotionEvent.ACTION_POINTER_UP:
                if (mode == Mode.NAV) {
                    navRef(e, idx);
                } else if (mode == Mode.DRAW && e.getPointerId(idx) == drawPointer) {
                    endDraw(e);
                }
                break;
            case MotionEvent.ACTION_UP:
                if (mode == Mode.DRAW) endDraw(e);
                else if (mode == Mode.NAV) endNav();
                mode = Mode.NONE;
                velocity.recycle();
                velocity = null;
                break;
            case MotionEvent.ACTION_CANCEL:
                cancelDraw();
                mode = Mode.NONE;
                pinching = false;
                if (velocity != null) {
                    velocity.recycle();
                    velocity = null;
                }
                invalidate();
                break;
            default:
                break;
        }
        return true;
    }

    // navigation

    private void startNav(MotionEvent e) {
        mode = Mode.NAV;
        navRef(e, -1);
    }

    private void navRef(MotionEvent e, int skip) {
        float sx = 0, sy = 0;
        int n = 0;
        for (int i = 0; i < e.getPointerCount(); i++) {
            if (i == skip) continue;
            sx += e.getX(i);
            sy += e.getY(i);
            n++;
        }
        if (n == 0) return;
        focusX = sx / n;
        focusY = sy / n;
        float d = 0;
        for (int i = 0; i < e.getPointerCount(); i++) {
            if (i == skip) continue;
            d += (float) Math.hypot(e.getX(i) - focusX, e.getY(i) - focusY);
        }
        span = n > 1 ? d / n : 0;
        pinching = n > 1;
    }

    private void moveNav(MotionEvent e) {
        int n = e.getPointerCount();
        float sx = 0, sy = 0;
        for (int i = 0; i < n; i++) {
            sx += e.getX(i);
            sy += e.getY(i);
        }
        float fx = sx / n, fy = sy / n;
        if (n > 1) {
            float d = 0;
            for (int i = 0; i < n; i++) d += (float) Math.hypot(e.getX(i) - fx, e.getY(i) - fy);
            float ns = d / n;
            float factor = span > 0 ? ns / span : 1;
            zoomAround(focusX, focusY, fx, fy, factor);
            span = ns;
        } else {
            offX += fx - focusX;
            offY += fy - focusY;
            clamp();
        }
        focusX = fx;
        focusY = fy;
        invalidate();
    }

    private void endNav() {
        if (!pinching && velocity != null) {
            velocity.computeCurrentVelocity(1000);
            float vx = velocity.getXVelocity(), vy = velocity.getYVelocity();
            int w = getWidth(), h = getHeight();
            float cw = docW * zoom, ch = docH * zoom;
            scroller.fling((int) offX, (int) offY, (int) vx, (int) vy,
                    (int) Math.min(0, w - cw), (int) Math.max(0, w - cw), (int) Math.min(0, h - ch), 0);
            postInvalidateOnAnimation();
        }
        pinching = false;
        invalidate();
    }

    @Override
    public void computeScroll() {
        if (scroller.computeScrollOffset()) {
            offX = scroller.getCurrX();
            offY = scroller.getCurrY();
            clamp();
            postInvalidateOnAnimation();
        }
    }

    // drawing

    private void startDraw(MotionEvent e, int idx, boolean stylus) {
        float[] o = new float[2];
        Doc.Page page = pageAtScreen(e.getX(idx), e.getY(idx), o);
        if (page == null) return;
        mode = Mode.DRAW;
        drawPointer = e.getPointerId(idx);
        drawByStylus = stylus;
        drawPage = page;
        drawTool = tools.tool;
        boolean eraserEnd = e.getToolType(idx) == MotionEvent.TOOL_TYPE_ERASER
                || (stylus && (e.getButtonState() & MotionEvent.BUTTON_STYLUS_PRIMARY) != 0);
        if (eraserEnd) drawTool = Tools.Tool.ERASER;
        startX = lastX = o[0];
        startY = lastY = o[1];
        downSX = e.getX(idx);
        downSY = e.getY(idx);
        moved = false;
        erased = false;
        npts = 0;
        nlasso = 0;
        selDrag = 0;
        usePressure = stylus && tools.pressure && drawTool == Tools.Tool.PEN;

        switch (drawTool) {
            case PEN:
            case HIGHLIGHTER: {
                live = new Stroke();
                boolean hl = drawTool == Tools.Tool.HIGHLIGHTER;
                live.tool = hl ? Stroke.HIGHLIGHTER : Stroke.PEN;
                live.color = hl ? (tools.hlColor & 0xffffff) | (tools.hlAlpha << 24)
                        : (tools.penColor & 0xffffff) | (tools.penAlpha << 24);
                live.width = hl ? tools.hlWidth() : tools.penWidth();
                live.style = hl ? null : Tools.STYLES[tools.penStyle];
                live.fill = tools.fill ? tools.fillAlpha : -1;
                live.cap = hl ? "butt" : "round";
                addPoint(o[0], o[1], e.getPressure(idx));
                break;
            }
            case SHAPE:
                live = new Stroke();
                live.tool = Stroke.PEN;
                live.color = (tools.penColor & 0xffffff) | (tools.penAlpha << 24);
                live.width = tools.penWidth();
                live.style = Tools.STYLES[tools.penStyle];
                live.fill = tools.fill && (tools.shape == Shapes.RECT || tools.shape == Shapes.ELLIPSE) ? tools.fillAlpha : -1;
                live.setPoints(Shapes.build(tools.shape, startX, startY, startX, startY, live.width), null);
                break;
            case ERASER:
                cursorX = o[0];
                cursorY = o[1];
                eraseAt(page, o[0], o[1]);
                break;
            case SELECT_RECT:
            case SELECT_LASSO:
                if (sel != null && sel.page == page) {
                    RectF b = sel.bounds();
                    float pad = 4 * density / zoom;
                    float handle = 22 * density / zoom;
                    if (Math.hypot(o[0] - (b.right + pad), o[1] - (b.bottom + pad)) < handle) {
                        selDrag = 2;
                    } else if (o[0] >= b.left - pad && o[0] <= b.right + pad && o[1] >= b.top - pad && o[1] <= b.bottom + pad) {
                        selDrag = 1;
                    }
                    if (selDrag != 0) {
                        beginSelectionEdit();
                        break;
                    }
                }
                clearSelection();
                addLasso(o[0], o[1]);
                if (drawTool == Tools.Tool.SELECT_RECT) addLasso(o[0], o[1]);
                break;
            default:
                break;
        }
        invalidate();
    }

    private void addPoint(float x, float y, float pressure) {
        if (npts > 0) {
            float dx = x - pts[2 * npts - 2], dy = y - pts[2 * npts - 1];
            float min = 1.2f / zoom;
            if (dx * dx + dy * dy < min * min) return;
        }
        if (2 * npts + 2 > pts.length) pts = Arrays.copyOf(pts, pts.length * 2);
        if (npts + 1 > prs.length) prs = Arrays.copyOf(prs, prs.length * 2);
        pts[2 * npts] = x;
        pts[2 * npts + 1] = y;
        prs[npts] = pressure;
        npts++;
        if (live != null) live.setPoints(Arrays.copyOf(pts, npts * 2), usePressure ? widths() : null);
    }

    private float[] widths() {
        float[] w = new float[npts];
        float base = live.width;
        for (int i = 0; i < npts; i++) {
            float p = Math.max(0f, Math.min(1f, prs[i]));
            w[i] = base * (0.3f + 0.9f * p);
        }
        return w;
    }

    private void addLasso(float x, float y) {
        if (2 * nlasso + 2 > lasso.length) lasso = Arrays.copyOf(lasso, lasso.length * 2);
        lasso[2 * nlasso] = x;
        lasso[2 * nlasso + 1] = y;
        nlasso++;
    }

    private void moveDraw(MotionEvent e) {
        int idx = e.findPointerIndex(drawPointer);
        if (idx < 0) return;
        if (!moved && Math.hypot(e.getX(idx) - downSX, e.getY(idx) - downSY) > touchSlop) moved = true;
        float[] o = new float[2];
        int hs = e.getHistorySize();
        for (int h = 0; h <= hs; h++) {
            float sx = h < hs ? e.getHistoricalX(idx, h) : e.getX(idx);
            float sy = h < hs ? e.getHistoricalY(idx, h) : e.getY(idx);
            float pr = h < hs ? e.getHistoricalPressure(idx, h) : e.getPressure(idx);
            toPage(drawPage, sx, sy, o);
            step(o[0], o[1], pr);
        }
        invalidate();
    }

    private void step(float x, float y, float pressure) {
        switch (drawTool) {
            case PEN:
            case HIGHLIGHTER:
                addPoint(x, y, pressure);
                break;
            case SHAPE:
                float ex = x, ey = y;
                if (tools.shape == Shapes.RECT || tools.shape == Shapes.ELLIPSE) {
                    // nothing special; free aspect
                } else if (tools.shape == Shapes.LINE || tools.shape == Shapes.ARROW || tools.shape == Shapes.DOUBLE_ARROW) {
                    // snap to 15 degree steps
                    double a = Math.atan2(y - startY, x - startX);
                    double snapped = Math.round(a / Math.toRadians(15)) * Math.toRadians(15);
                    if (Math.abs(a - snapped) < Math.toRadians(3)) {
                        float len = (float) Math.hypot(x - startX, y - startY);
                        ex = startX + (float) (len * Math.cos(snapped));
                        ey = startY + (float) (len * Math.sin(snapped));
                    }
                }
                live.setPoints(Shapes.build(tools.shape, startX, startY, ex, ey, live.width), null);
                break;
            case ERASER:
                cursorX = x;
                cursorY = y;
                eraseAt(drawPage, x, y);
                break;
            case SELECT_RECT:
            case SELECT_LASSO:
                if (selDrag == 1) {
                    float dx = x - lastX, dy = y - lastY;
                    for (Element el : sel.items) el.translate(dx, dy);
                } else if (selDrag == 2) {
                    RectF b = sel.bounds();
                    float w = Math.max(b.width(), 1), h = Math.max(b.height(), 1);
                    float f = Math.max((x - b.left) / w, (y - b.top) / h);
                    float minF = 8f / Math.max(w, h);
                    f = Math.max(f, minF);
                    if (Math.abs(f - 1) > 0.002f) {
                        for (Element el : sel.items) el.scale(f, f, b.left, b.top);
                    }
                } else if (drawTool == Tools.Tool.SELECT_RECT) {
                    lasso[2] = x;
                    lasso[3] = y;
                } else {
                    addLasso(x, y);
                }
                break;
            default:
                break;
        }
        lastX = x;
        lastY = y;
    }

    private void endDraw(MotionEvent e) {
        Doc.Page page = drawPage;
        Doc.Layer layer = page == null ? null : page.layer();
        switch (drawTool) {
            case PEN:
            case HIGHLIGHTER:
                if (live != null && npts > 0) {
                    if (tools.recognizer && drawTool == Tools.Tool.PEN) {
                        float[] r = Shapes.recognize(live);
                        if (r != null) live.setPoints(r, null);
                    }
                    addElement(page, live);
                }
                break;
            case SHAPE:
                if (live != null && moved) addElement(page, live);
                break;
            case ERASER:
                if (erased) {
                    undo.commit();
                    host.onEdited();
                }
                cursorX = Float.NaN;
                break;
            case TEXT:
                if (!moved && layer != null) {
                    TextEl hit = null;
                    for (Element el : layer.elements) {
                        if (el instanceof TextEl && el.hit(startX, startY, 4)) hit = (TextEl) el;
                    }
                    host.onTextTap(page, startX, startY, hit);
                }
                break;
            case SELECT_RECT:
            case SELECT_LASSO:
                if (selDrag != 0) {
                    if (undo.commit()) host.onEdited();
                } else if (layer != null) {
                    finishSelection(page, layer);
                }
                selDrag = 0;
                nlasso = 0;
                break;
            default:
                break;
        }
        live = null;
        drawPage = null;
        mode = Mode.NONE;
        invalidate();
    }

    private void cancelDraw() {
        if (mode == Mode.DRAW && drawTool == Tools.Tool.ERASER && erased) {
            undo.commit();
            host.onEdited();
        }
        if (mode == Mode.DRAW && selDrag != 0 && undo.commit()) host.onEdited();
        live = null;
        drawPage = null;
        nlasso = 0;
        selDrag = 0;
        cursorX = Float.NaN;
        mode = Mode.NONE;
    }

    private void addElement(Doc.Page page, Element el) {
        Doc.Layer layer = page.layer();
        undo.touch(layer);
        layer.elements.add(el);
        undo.commit();
        host.onEdited();
    }

    // eraser

    private void eraseAt(Doc.Page page, float x, float y) {
        Doc.Layer layer = page.layer();
        float r = tools.eraserWidth() / 2;
        List<Element> list = layer.elements;
        for (int i = list.size() - 1; i >= 0; i--) {
            Element el = list.get(i);
            if (!(el instanceof Stroke)) continue;
            Stroke s = (Stroke) el;
            if (!s.hit(x, y, r)) continue;
            undo.touch(layer);
            erased = true;
            if (tools.eraseWhole || s.count() <= 2) {
                list.remove(i);
                continue;
            }
            boolean[] near = s.pointsNear(x, y, r);
            List<Stroke> parts = new ArrayList<>();
            int from = -1;
            for (int k = 0; k <= near.length; k++) {
                boolean keep = k < near.length && !near[k];
                if (keep && from < 0) from = k;
                if (!keep && from >= 0) {
                    if (k - from >= 2) parts.add(s.slice(from, k));
                    from = -1;
                }
            }
            list.remove(i);
            list.addAll(i, parts);
        }
    }

    // selection

    private void finishSelection(Doc.Page page, Doc.Layer layer) {
        List<Element> picked = new ArrayList<>();
        if (!moved) {
            for (int i = layer.elements.size() - 1; i >= 0; i--) {
                Element el = layer.elements.get(i);
                if (el.hit(startX, startY, 6 * density / zoom)) {
                    picked.add(el);
                    break;
                }
            }
        } else if (drawTool == Tools.Tool.SELECT_RECT) {
            RectF r = new RectF(Math.min(lasso[0], lasso[2]), Math.min(lasso[1], lasso[3]),
                    Math.max(lasso[0], lasso[2]), Math.max(lasso[1], lasso[3]));
            for (Element el : layer.elements) {
                RectF b = el.bounds();
                if (r.contains(b.centerX(), b.centerY()) && RectF.intersects(r, b)) {
                    if (el instanceof Stroke) {
                        if (fractionInside((Stroke) el, r, null) >= 0.9f) picked.add(el);
                    } else if (r.contains(b)) {
                        picked.add(el);
                    }
                }
            }
        } else if (nlasso >= 3) {
            for (Element el : layer.elements) {
                if (el instanceof Stroke) {
                    if (fractionInside((Stroke) el, null, lasso) >= 0.9f) picked.add(el);
                } else {
                    RectF b = el.bounds();
                    if (inPolygon(b.centerX(), b.centerY())) picked.add(el);
                }
            }
        }
        if (picked.isEmpty()) {
            clearSelection();
        } else {
            sel = new Selection(page, layer, picked);
            host.onSelectionChanged(true);
        }
    }

    private float fractionInside(Stroke s, RectF r, float[] poly) {
        int n = s.count(), in = 0;
        for (int i = 0; i < n; i++) {
            float x = s.pts[2 * i], y = s.pts[2 * i + 1];
            if (r != null ? r.contains(x, y) : inPolygon(x, y)) in++;
        }
        return n == 0 ? 0 : in / (float) n;
    }

    private boolean inPolygon(float x, float y) {
        boolean c = false;
        for (int i = 0, j = nlasso - 1; i < nlasso; j = i++) {
            float xi = lasso[2 * i], yi = lasso[2 * i + 1], xj = lasso[2 * j], yj = lasso[2 * j + 1];
            if ((yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi) c = !c;
        }
        return c;
    }

    /** Replaces the selected elements with copies so they can be changed without breaking undo. */
    private void beginSelectionEdit() {
        undo.touch(sel.layer);
        List<Element> copies = new ArrayList<>();
        for (Element el : sel.items) {
            int i = indexOf(sel.layer.elements, el);
            Element c = el.copy();
            if (i >= 0) sel.layer.elements.set(i, c);
            copies.add(c);
        }
        sel.items = copies;
    }

    private static int indexOf(List<Element> list, Element el) {
        for (int i = 0; i < list.size(); i++) if (list.get(i) == el) return i;
        return -1;
    }

    public boolean hasSelection() {
        return sel != null;
    }

    public void clearSelection() {
        if (sel != null) {
            sel = null;
            if (host != null) host.onSelectionChanged(false);
        }
        invalidate();
    }

    public void deleteSelection() {
        if (sel == null) return;
        undo.touch(sel.layer);
        for (Element el : sel.items) {
            int i = indexOf(sel.layer.elements, el);
            if (i >= 0) sel.layer.elements.remove(i);
        }
        undo.commit();
        clearSelection();
        host.onEdited();
    }

    public void copySelection() {
        if (sel == null) return;
        CLIPBOARD.clear();
        for (Element el : sel.items) CLIPBOARD.add(el.copy());
    }

    public void cutSelection() {
        copySelection();
        deleteSelection();
    }

    public static boolean canPaste() {
        return !CLIPBOARD.isEmpty();
    }

    public void paste() {
        if (CLIPBOARD.isEmpty()) return;
        Doc.Page page = doc.pages.get(currentPage());
        Doc.Layer layer = page.layer();
        undo.touch(layer);
        List<Element> added = new ArrayList<>();
        for (Element el : CLIPBOARD) {
            Element c = el.copy();
            layer.elements.add(c);
            added.add(c);
        }
        undo.commit();
        sel = new Selection(page, layer, added);
        host.onSelectionChanged(true);
        host.onEdited();
        invalidate();
    }

    public void duplicateSelection() {
        if (sel == null) return;
        undo.touch(sel.layer);
        List<Element> added = new ArrayList<>();
        for (Element el : sel.items) {
            Element c = el.copy();
            c.translate(12, 12);
            sel.layer.elements.add(c);
            added.add(c);
        }
        undo.commit();
        sel.items = added;
        host.onEdited();
        invalidate();
    }

    public void recolorSelection(int color) {
        if (sel == null) return;
        beginSelectionEdit();
        for (Element el : sel.items) {
            if (el instanceof Stroke) ((Stroke) el).setColor(color);
            else el.setColor(color);
        }
        if (undo.commit()) host.onEdited();
        invalidate();
    }

    public void bringSelectionToFront() {
        if (sel == null) return;
        undo.touch(sel.layer);
        for (Element el : sel.items) {
            int i = indexOf(sel.layer.elements, el);
            if (i >= 0) sel.layer.elements.remove(i);
        }
        sel.layer.elements.addAll(sel.items);
        if (undo.commit()) host.onEdited();
        invalidate();
    }

    // ------------------------------------------------------------------ edits from the activity

    public void putText(Doc.Page page, TextEl old, TextEl now) {
        Doc.Layer layer = page.layer();
        undo.touch(layer);
        int i = old == null ? -1 : indexOf(layer.elements, old);
        if (now == null) {
            if (i >= 0) layer.elements.remove(i);
        } else if (i >= 0) {
            layer.elements.set(i, now);
        } else {
            layer.elements.add(now);
        }
        if (undo.commit()) host.onEdited();
        invalidate();
    }

    /** Places an image in the middle of the visible part of the current page and selects it. */
    public void insertImage(Bitmap b) {
        Doc.Page page = doc.pages.get(currentPage());
        float maxW = page.width * 0.6f, maxH = page.height * 0.5f;
        float s = Math.min(maxW / b.getWidth(), maxH / b.getHeight());
        s = Math.min(s, 1f) * 1f;
        float w = b.getWidth() * s, h = b.getHeight() * s;
        float[] o = new float[2];
        toPage(page, getWidth() / 2f, getHeight() / 2f, o);
        float cx = Math.max(w / 2, Math.min(page.width - w / 2, o[0]));
        float cy = Math.max(h / 2, Math.min(page.height - h / 2, o[1]));
        ImageEl im = ImageEl.fromBitmap(b);
        im.left = cx - w / 2;
        im.top = cy - h / 2;
        im.right = cx + w / 2;
        im.bottom = cy + h / 2;
        Doc.Layer layer = page.layer();
        undo.touch(layer);
        layer.elements.add(im);
        undo.commit();
        List<Element> one = new ArrayList<>();
        one.add(im);
        sel = new Selection(page, layer, one);
        host.onSelectionChanged(true);
        host.onEdited();
        invalidate();
    }

    public void addPage(int at, Doc.Page p) {
        undo.touchPages();
        doc.pages.add(at, p);
        undo.commit();
        relayout();
        host.onEdited();
    }

    public void deletePage(int i) {
        if (doc.pages.size() <= 1 || i < 0 || i >= doc.pages.size()) return;
        undo.touchPages();
        doc.pages.remove(i);
        undo.commit();
        relayout();
        host.onEdited();
    }

    public void movePage(int i, int delta) {
        int j = i + delta;
        if (i < 0 || j < 0 || i >= doc.pages.size() || j >= doc.pages.size()) return;
        undo.touchPages();
        Doc.Page p = doc.pages.remove(i);
        doc.pages.add(j, p);
        undo.commit();
        relayout();
        scrollToPage(j);
        host.onEdited();
    }

    public void afterUndoRedo() {
        clearSelection();
        relayout();
    }

    /** Renders one page with its ink, for thumbnails and the file preview. */
    public Bitmap renderPage(int index, int maxW) {
        Doc.Page p = doc.pages.get(index);
        float s = maxW / p.width;
        int w = Math.max(1, Math.round(p.width * s)), h = Math.max(1, Math.round(p.height * s));
        Bitmap b = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888);
        Canvas c = new Canvas(b);
        c.scale(s, s);
        Doc.Background bg = p.bg;
        if (bg.type == Doc.Background.Type.PDF && pdf != null && !doc.pdfMissing) {
            c.drawColor(0xffffffff);
            PdfPages.Rendered r = pdf.cached(bg.pdfPage);
            Bitmap src = r != null ? r.bitmap : pdf.renderNow(bg.pdfPage, s * 1.5f, p.width, p.height);
            if (src != null) c.drawBitmap(src, null, new RectF(0, 0, p.width, p.height), bmpPaint);
        } else {
            boolean wasPinching = pinching;
            pinching = true;
            drawBackground(c, p);
            pinching = wasPinching;
        }
        for (Doc.Layer layer : p.layers) {
            if (!layer.visible) continue;
            for (Element e : layer.elements) e.draw(c);
        }
        return b;
    }
}
