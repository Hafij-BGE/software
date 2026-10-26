package com.hafij.inkwell;

import android.content.Context;
import android.content.SharedPreferences;

import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

import java.util.ArrayList;
import java.util.List;

/** Recently opened documents of any kind, shown in the Home "Recent" row. */
public final class Recents {
    private static final String PREFS = "inkwell";
    private static final String KEY = "recent";
    private static final int MAX = 16;

    private Recents() {
    }

    public static final class Item {
        public final String uri;
        public final String name;
        public final long time;

        Item(String uri, String name, long time) {
            this.uri = uri;
            this.name = name;
            this.time = time;
        }
    }

    public static synchronized List<Item> list(Context c) {
        List<Item> out = new ArrayList<>();
        try {
            JSONArray a = new JSONArray(prefs(c).getString(KEY, "[]"));
            for (int i = 0; i < a.length(); i++) {
                JSONObject o = a.getJSONObject(i);
                out.add(new Item(o.getString("uri"), o.optString("name"), o.optLong("time")));
            }
        } catch (JSONException ignored) {
            // Start over.
        }
        return out;
    }

    /** Called when any document is opened: Markdown in Markd's editor or a note in Inkwell. */
    public static synchronized void add(Context c, String uri, String name) {
        if (uri == null || name == null) return;
        List<Item> items = list(c);
        items.removeIf(i -> i.uri.equals(uri));
        items.add(0, new Item(uri, name, System.currentTimeMillis()));
        save(c, items);
    }

    public static synchronized void remove(Context c, String uri) {
        List<Item> items = list(c);
        items.removeIf(i -> i.uri.equals(uri));
        save(c, items);
    }

    public static synchronized void rename(Context c, String oldUri, String newUri, String name) {
        List<Item> items = list(c);
        for (int i = 0; i < items.size(); i++) {
            if (items.get(i).uri.equals(oldUri)) items.set(i, new Item(newUri, name, items.get(i).time));
        }
        save(c, items);
    }

    private static void save(Context c, List<Item> items) {
        JSONArray a = new JSONArray();
        try {
            for (int i = 0; i < items.size() && i < MAX; i++) {
                Item it = items.get(i);
                a.put(new JSONObject().put("uri", it.uri).put("name", it.name).put("time", it.time));
            }
        } catch (JSONException ignored) {
            return;
        }
        prefs(c).edit().putString(KEY, a.toString()).apply();
    }

    private static SharedPreferences prefs(Context c) {
        return c.getSharedPreferences(PREFS, Context.MODE_PRIVATE);
    }
}
