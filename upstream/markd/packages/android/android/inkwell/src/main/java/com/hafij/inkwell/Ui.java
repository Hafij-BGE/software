package com.hafij.inkwell;

import android.content.Context;
import android.content.res.TypedArray;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
import android.content.res.ColorStateList;
import android.util.TypedValue;
import android.view.Gravity;
import android.view.View;
import android.widget.ImageButton;
import android.widget.TextView;

/** Small helpers for building the UI in code. */
public final class Ui {
    private Ui() {
    }

    public static int dp(Context c, float v) {
        return Math.round(TypedValue.applyDimension(TypedValue.COMPLEX_UNIT_DIP, v, c.getResources().getDisplayMetrics()));
    }

    public static int attr(Context c, int attr) {
        TypedArray a = c.obtainStyledAttributes(new int[]{attr});
        int v = a.getColor(0, 0xff808080);
        a.recycle();
        return v;
    }

    public static int accent(Context c) {
        return attr(c, android.R.attr.colorAccent);
    }

    public static int text(Context c) {
        return attr(c, android.R.attr.textColorPrimary);
    }

    public static int surface(Context c) {
        return attr(c, android.R.attr.colorBackground);
    }

    public static ImageButton icon(Context c, int res, String desc, View.OnClickListener l) {
        ImageButton b = new ImageButton(c);
        b.setImageResource(res);
        b.setContentDescription(desc);
        b.setTooltipText(desc);
        int p = dp(c, 10);
        b.setPadding(p, p, p, p);
        b.setScaleType(android.widget.ImageView.ScaleType.FIT_CENTER);
        b.setBackground(ripple(c, 0));
        b.setOnClickListener(l);
        b.setLayoutParams(new android.widget.LinearLayout.LayoutParams(dp(c, 44), dp(c, 44)));
        return b;
    }

    /** Ripple with an optional filled rounded background (0 = transparent). */
    public static android.graphics.drawable.Drawable ripple(Context c, int fill) {
        GradientDrawable shape = new GradientDrawable();
        shape.setCornerRadius(dp(c, 10));
        shape.setColor(fill);
        GradientDrawable mask = new GradientDrawable();
        mask.setCornerRadius(dp(c, 10));
        mask.setColor(0xffffffff);
        return new RippleDrawable(ColorStateList.valueOf(0x33808080), shape, mask);
    }

    public static TextView label(Context c, String s, float sp) {
        TextView t = new TextView(c);
        t.setText(s);
        t.setTextSize(sp);
        t.setTextColor(text(c));
        t.setGravity(Gravity.CENTER_VERTICAL);
        t.setSingleLine(true);
        t.setEllipsize(android.text.TextUtils.TruncateAt.END);
        return t;
    }

    public static int withAlpha(int color, int alpha) {
        return (color & 0xffffff) | (alpha << 24);
    }
}
