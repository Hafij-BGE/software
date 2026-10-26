package com.hafij.inkwell.model;

import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.util.Base64;

import java.io.ByteArrayOutputStream;

/** An embedded image (PNG/JPEG bytes, base64 in the file). Also used for TeX images. */
public class ImageEl extends Element {
    public float left, top, right, bottom;
    /** Base64 image data exactly as stored in the note file. */
    public String data = "";
    /** Non-null for &lt;teximage&gt;: the TeX source, kept for round-tripping. */
    public String tex;

    private transient Bitmap bitmap;
    private transient boolean decodeFailed;
    private static final Paint PAINT = new Paint(Paint.FILTER_BITMAP_FLAG | Paint.ANTI_ALIAS_FLAG);
    private static final Paint MISSING = new Paint();

    public static ImageEl fromBitmap(Bitmap b) {
        ImageEl i = new ImageEl();
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        b.compress(b.hasAlpha() ? Bitmap.CompressFormat.PNG : Bitmap.CompressFormat.JPEG, 92, out);
        i.data = Base64.encodeToString(out.toByteArray(), Base64.NO_WRAP);
        i.bitmap = b;
        return i;
    }

    public byte[] bytes() {
        try {
            return Base64.decode(data, Base64.DEFAULT);
        } catch (IllegalArgumentException e) {
            return new byte[0];
        }
    }

    public Bitmap bitmap() {
        if (bitmap == null && !decodeFailed) {
            byte[] b = bytes();
            bitmap = BitmapFactory.decodeByteArray(b, 0, b.length);
            decodeFailed = bitmap == null;
        }
        return bitmap;
    }

    @Override
    public RectF bounds() {
        return new RectF(Math.min(left, right), Math.min(top, bottom), Math.max(left, right), Math.max(top, bottom));
    }

    @Override
    public void draw(Canvas c) {
        Bitmap b = bitmap();
        if (b != null) {
            c.drawBitmap(b, null, bounds(), PAINT);
        } else {
            MISSING.setColor(0x33888888);
            c.drawRect(bounds(), MISSING);
        }
    }

    @Override
    public void translate(float dx, float dy) {
        left += dx;
        right += dx;
        top += dy;
        bottom += dy;
    }

    @Override
    public void scale(float sx, float sy, float px, float py) {
        left = px + (left - px) * sx;
        right = px + (right - px) * sx;
        top = py + (top - py) * sy;
        bottom = py + (bottom - py) * sy;
    }

    @Override
    public Element copy() {
        ImageEl i = new ImageEl();
        i.left = left;
        i.top = top;
        i.right = right;
        i.bottom = bottom;
        i.data = data;
        i.tex = tex;
        i.bitmap = bitmap;
        i.decodeFailed = decodeFailed;
        return i;
    }

    @Override
    public boolean hit(float x, float y, float radius) {
        RectF b = bounds();
        return x >= b.left - radius && x <= b.right + radius && y >= b.top - radius && y <= b.bottom + radius;
    }

    @Override
    public int getColor() {
        return 0;
    }

    @Override
    public void setColor(int argb) {
    }
}
