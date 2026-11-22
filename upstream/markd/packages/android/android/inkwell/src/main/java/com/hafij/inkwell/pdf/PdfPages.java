package com.hafij.inkwell.pdf;

import android.graphics.Bitmap;
import android.graphics.Color;
import android.graphics.pdf.PdfRenderer;
import android.os.Handler;
import android.os.Looper;
import android.os.ParcelFileDescriptor;
import android.util.LruCache;

import java.io.File;
import java.io.IOException;
import java.util.HashSet;
import java.util.Set;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

/** Renders background PDF pages on one worker thread and caches the bitmaps. */
public class PdfPages {
    public interface Listener {
        void onRendered(int page);
    }

    public static final class Rendered {
        public final Bitmap bitmap;
        /** Pixels per PDF point. */
        public final float scale;

        Rendered(Bitmap bitmap, float scale) {
            this.bitmap = bitmap;
            this.scale = scale;
        }
    }

    private final ExecutorService worker = Executors.newSingleThreadExecutor();
    private final Handler main = new Handler(Looper.getMainLooper());
    private final Set<Integer> pending = new HashSet<>();
    private final LruCache<Integer, Rendered> cache;
    private final Listener listener;
    private PdfRenderer renderer;
    private ParcelFileDescriptor fd;
    private int count;
    private volatile boolean closed;

    public PdfPages(Listener listener) {
        this.listener = listener;
        int max = (int) Math.min(Runtime.getRuntime().maxMemory() / 5, 160L * 1024 * 1024);
        cache = new LruCache<Integer, Rendered>(max) {
            @Override
            protected int sizeOf(Integer key, Rendered value) {
                return value.bitmap.getByteCount();
            }
        };
    }

    /** Opens the file synchronously on the worker; returns the page count. */
    public int open(File f) throws IOException {
        try {
            return worker.submit(() -> {
                closeRenderer();
                fd = ParcelFileDescriptor.open(f, ParcelFileDescriptor.MODE_READ_ONLY);
                renderer = new PdfRenderer(fd);
                count = renderer.getPageCount();
                return count;
            }).get();
        } catch (Exception e) {
            throw new IOException(e.getCause() != null ? e.getCause().getMessage() : e.getMessage(), e);
        }
    }

    public int count() {
        return count;
    }

    public Rendered cached(int page) {
        return cache.get(page);
    }

    /** Requests a render of {@code page} at about {@code scale} px/pt (capped for memory). */
    public void request(int page, float scale, float wPt, float hPt) {
        if (renderer == null || page < 0 || page >= count) return;
        float s = cap(scale, wPt, hPt);
        Rendered r = cache.get(page);
        if (r != null && Math.abs(r.scale - s) / s < 0.15f) return;
        if (!pending.add(page)) return;
        worker.execute(() -> {
            Bitmap b = render(page, s, wPt, hPt);
            main.post(() -> {
                pending.remove(page);
                if (closed || b == null) return;
                cache.put(page, new Rendered(b, s));
                listener.onRendered(page);
            });
        });
    }

    private static float cap(float scale, float w, float h) {
        float s = Math.max(scale, 0.25f);
        float maxSide = 4096f;
        s = Math.min(s, maxSide / Math.max(w, h));
        float maxPixels = 12_000_000f;
        if (w * h * s * s > maxPixels) s = (float) Math.sqrt(maxPixels / (w * h));
        return s;
    }

    private Bitmap render(int index, float s, float wPt, float hPt) {
        if (renderer == null || closed) return null;
        int w = Math.max(1, Math.round(wPt * s));
        int h = Math.max(1, Math.round(hPt * s));
        try (PdfRenderer.Page p = renderer.openPage(index)) {
            Bitmap b = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888);
            b.eraseColor(Color.WHITE);
            p.render(b, null, null, PdfRenderer.Page.RENDER_MODE_FOR_DISPLAY);
            return b;
        } catch (RuntimeException | OutOfMemoryError e) {
            return null;
        }
    }

    /** Blocking render for thumbnails; safe to call from the UI thread for small sizes. */
    public Bitmap renderNow(int index, float s, float wPt, float hPt) {
        if (renderer == null || index < 0 || index >= count) return null;
        try {
            return worker.submit(() -> render(index, s, wPt, hPt)).get();
        } catch (Exception e) {
            return null;
        }
    }

    private void closeRenderer() {
        try {
            if (renderer != null) renderer.close();
            if (fd != null) fd.close();
        } catch (IOException | RuntimeException ignored) {
            // already closed
        }
        renderer = null;
        fd = null;
    }

    public void close() {
        closed = true;
        cache.evictAll();
        worker.execute(this::closeRenderer);
        worker.shutdown();
    }
}
