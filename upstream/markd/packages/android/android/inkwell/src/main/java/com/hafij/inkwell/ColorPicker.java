package com.hafij.inkwell;

import android.app.AlertDialog;
import android.content.Context;
import android.graphics.Color;
import android.graphics.drawable.GradientDrawable;
import android.text.Editable;
import android.text.TextWatcher;
import android.view.View;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.SeekBar;
import android.widget.TextView;

/** Hue / saturation / value picker with a hex field. */
public final class ColorPicker {
    public interface Callback {
        void onColor(int argb);
    }

    private ColorPicker() {
    }

    public static void show(Context c, int initial, Callback cb) {
        float[] hsv = new float[3];
        Color.colorToHSV(initial, hsv);
        LinearLayout box = new LinearLayout(c);
        box.setOrientation(LinearLayout.VERTICAL);
        int pad = Ui.dp(c, 20);
        box.setPadding(pad, pad / 2, pad, 0);

        View preview = new View(c);
        GradientDrawable pd = new GradientDrawable();
        pd.setCornerRadius(Ui.dp(c, 12));
        preview.setBackground(pd);
        box.addView(preview, new LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, Ui.dp(c, 48)));

        EditText hex = new EditText(c);
        hex.setSingleLine(true);
        hex.setHint("#RRGGBB");

        SeekBar[] bars = new SeekBar[3];
        String[] names = {"Hue", "Saturation", "Brightness"};
        int[] max = {360, 100, 100};
        boolean[] updating = {false};
        Runnable refresh = () -> {
            int col = Color.HSVToColor(hsv);
            pd.setColor(col);
            if (!updating[0]) {
                updating[0] = true;
                hex.setText(String.format("#%06X", col & 0xffffff));
                updating[0] = false;
            }
        };
        for (int i = 0; i < 3; i++) {
            TextView t = Ui.label(c, names[i], 13);
            t.setPadding(0, Ui.dp(c, 10), 0, 0);
            box.addView(t);
            SeekBar s = new SeekBar(c);
            s.setMax(max[i]);
            s.setProgress(Math.round(i == 0 ? hsv[0] : hsv[i] * 100));
            final int k = i;
            s.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
                @Override
                public void onProgressChanged(SeekBar sb, int p, boolean user) {
                    if (!user) return;
                    hsv[k] = k == 0 ? p : p / 100f;
                    refresh.run();
                }

                @Override
                public void onStartTrackingTouch(SeekBar sb) {
                }

                @Override
                public void onStopTrackingTouch(SeekBar sb) {
                }
            });
            bars[i] = s;
            box.addView(s);
        }
        box.addView(hex);
        hex.addTextChangedListener(new TextWatcher() {
            @Override
            public void beforeTextChanged(CharSequence s, int a, int b, int d) {
            }

            @Override
            public void onTextChanged(CharSequence s, int a, int b, int d) {
            }

            @Override
            public void afterTextChanged(Editable e) {
                if (updating[0]) return;
                try {
                    int col = Color.parseColor(e.toString().trim());
                    Color.colorToHSV(col, hsv);
                    updating[0] = true;
                    bars[0].setProgress(Math.round(hsv[0]));
                    bars[1].setProgress(Math.round(hsv[1] * 100));
                    bars[2].setProgress(Math.round(hsv[2] * 100));
                    pd.setColor(col);
                    updating[0] = false;
                } catch (IllegalArgumentException ignored) {
                    // incomplete input
                }
            }
        });
        refresh.run();
        new AlertDialog.Builder(c)
                .setTitle("Custom colour")
                .setView(box)
                .setPositiveButton(android.R.string.ok, (d, w) -> cb.onColor(Color.HSVToColor(hsv)))
                .setNegativeButton(android.R.string.cancel, null)
                .show();
    }
}
