// The game's app on Android. The game itself is the engine's viewer, in the app's native
// library (apps/viewer/src/android.rs), which draws on this activity's window. This is what the
// system has to be asked for in Java:
//
// - the game's files, which are packed in the app and are unpacked here into the app's own
//   folder the first time, where the game reads them as it does on a desktop;
// - the game's pages, its menus and instruments, in a web view laid over the game;
// - a controller, and the Back key, which are heard here and told to the game as the pages
//   tell it things, a line each (docs/ui.md);
// - what the game fetches from its site, its maps, with the system's own web client.
//
// The game calls the methods below from its own threads.
package com.rust4x4.outback;

import android.app.Dialog;
import android.app.NativeActivity;
import android.content.res.AssetManager;
import android.graphics.Color;
import android.hardware.input.InputManager;
import android.os.Build;
import android.os.Bundle;
import android.util.Log;
import android.view.InputDevice;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.view.Window;
import android.view.WindowManager;
import android.webkit.ConsoleMessage;
import android.webkit.JavascriptInterface;
import android.webkit.WebChromeClient;
import android.webkit.WebSettings;
import android.webkit.WebView;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;

public class MainActivity extends NativeActivity implements InputManager.InputDeviceListener {
    private static final String TAG = "solstice";
    /** Where the game's files are in the app's package. */
    private static final String PACKED = "game";

    /** What the pages have said since the game last asked. */
    private final ArrayList<String> said = new ArrayList<>();
    private WebView pages;
    private Dialog over;
    private volatile String fetchError = "";

    @Override
    protected void onCreate(Bundle saved) {
        // Before the game starts, which it does in NativeActivity's onCreate.
        unpack();
        super.onCreate(saved);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        wholeScreen(getWindow());
    }

    @Override
    public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        // The system's bars come back whenever they are swiped for.
        if (focused) {
            wholeScreen(getWindow());
            if (over != null) wholeScreen(over.getWindow());
        }
    }

    @Override
    protected void onDestroy() {
        if (over != null) over.dismiss();
        over = null;
        pages = null;
        super.onDestroy();
        // The game cannot be started twice in one process: the next opening is a new one.
        android.os.Process.killProcess(android.os.Process.myPid());
    }

    /** The game is over: closes the app. */
    public void quit() {
        runOnUiThread(this::finishAndRemoveTask);
    }

    /** Gives a window the whole screen: no bars, and into the corners round a camera. */
    @SuppressWarnings("deprecation")
    private static void wholeScreen(Window window) {
        if (window == null) return;
        if (Build.VERSION.SDK_INT >= 28) {
            WindowManager.LayoutParams how = window.getAttributes();
            how.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
            window.setAttributes(how);
        }
        window.getDecorView().setSystemUiVisibility(
                View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
                        | View.SYSTEM_UI_FLAG_FULLSCREEN
                        | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
                        | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
                        | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN
                        | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION);
    }

    // ---- The game's files.

    /** The app's own folder: the game's files, its maps, and what it keeps. */
    private File folder() {
        File folder = getExternalFilesDir(null);
        return folder != null ? folder : getFilesDir();
    }

    /**
     * Unpacks the game's files into the app's folder, unless this build of the app has done so
     * already. Its maps are not among them: the game fetches those itself.
     */
    private void unpack() {
        File folder = folder();
        String build;
        try {
            build = String.valueOf(getPackageManager().getPackageInfo(getPackageName(), 0).lastUpdateTime);
        } catch (Exception error) {
            build = "unknown";
        }
        File mark = new File(folder, "unpacked.txt");
        try {
            if (mark.exists() && build.equals(new String(read(new FileInputStream(mark)), StandardCharsets.UTF_8))) {
                return;
            }
            copy(getAssets(), PACKED, folder);
            try (OutputStream out = new FileOutputStream(mark)) {
                out.write(build.getBytes(StandardCharsets.UTF_8));
            }
        } catch (IOException error) {
            Log.e(TAG, "the game's files could not be unpacked", error);
        }
    }

    private static void copy(AssetManager packed, String path, File to) throws IOException {
        String[] inside = packed.list(path);
        if (inside != null && inside.length > 0) {
            if (!to.isDirectory() && !to.mkdirs()) throw new IOException("no folder " + to);
            for (String name : inside) copy(packed, path + "/" + name, new File(to, name));
            return;
        }
        try (InputStream in = packed.open(path); OutputStream out = new FileOutputStream(to)) {
            pour(in, out);
        }
    }

    private static void pour(InputStream in, OutputStream out) throws IOException {
        byte[] some = new byte[1 << 16];
        for (int count; (count = in.read(some)) > 0; ) out.write(some, 0, count);
    }

    private static byte[] read(InputStream in) throws IOException {
        try (InputStream from = in) {
            ByteArrayOutputStream all = new ByteArrayOutputStream();
            pour(from, all);
            return all.toByteArray();
        }
    }

    // ---- The pages.

    /** What a page calls to say something to the game: `window.ipc.postMessage(line)`. */
    private final class Ipc {
        @JavascriptInterface
        public void postMessage(String line) {
            say(line);
        }
    }

    /**
     * Lays the page at an address over the game. The view is in a window of its own on top of
     * the activity's, which is the game's to draw on and shows no views of its own. It is
     * see-through wherever its page is, and takes every touch; the keys and a controller come
     * to it too, and are passed on to the game.
     */
    public void overlayOpen(final String address) {
        runOnUiThread(() -> {
            if (pages == null) {
                pages = new WebView(this);
                pages.setBackgroundColor(Color.TRANSPARENT);
                pages.setOverScrollMode(View.OVER_SCROLL_NEVER);
                pages.setVerticalScrollBarEnabled(false);
                pages.setHorizontalScrollBarEnabled(false);
                WebSettings settings = pages.getSettings();
                settings.setJavaScriptEnabled(true);
                settings.setAllowFileAccess(true);
                settings.setDomStorageEnabled(true);
                settings.setSupportZoom(false);
                settings.setTextZoom(100);
                pages.addJavascriptInterface(new Ipc(), "ipc");
                pages.setWebChromeClient(new WebChromeClient() {
                    @Override
                    public boolean onConsoleMessage(ConsoleMessage message) {
                        Log.i(TAG, "page: " + message.message() + " (" + message.sourceId() + ":" + message.lineNumber() + ")");
                        return true;
                    }
                });
                // The window in front is the one the keys and a controller go to.
                over = new Dialog(this, android.R.style.Theme_Translucent_NoTitleBar_Fullscreen) {
                    @Override
                    public boolean dispatchKeyEvent(KeyEvent event) {
                        return heardKey(event) || super.dispatchKeyEvent(event);
                    }

                    @Override
                    public boolean dispatchGenericMotionEvent(MotionEvent event) {
                        return heardSticks(event) || super.dispatchGenericMotionEvent(event);
                    }
                };
                over.setCancelable(false);
                over.setContentView(pages, new ViewGroup.LayoutParams(
                        ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
                Window window = over.getWindow();
                window.clearFlags(WindowManager.LayoutParams.FLAG_DIM_BEHIND);
                window.addFlags(WindowManager.LayoutParams.FLAG_LAYOUT_IN_SCREEN
                        | WindowManager.LayoutParams.FLAG_LAYOUT_NO_LIMITS
                        | WindowManager.LayoutParams.FLAG_HARDWARE_ACCELERATED);
                window.setLayout(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT);
                wholeScreen(window);
                over.show();
                // A controller already plugged in, and any that comes or goes.
                InputManager devices = (InputManager) getSystemService(INPUT_SERVICE);
                devices.registerInputDeviceListener(this, null);
                if (controllers() > 0) padHere();
            }
            pages.loadUrl(address);
        });
    }

    private void say(String line) {
        synchronized (said) {
            said.add(line);
        }
    }

    // ---- A controller. The game hears of it as a page would tell it: `pad button NAME VALUE`
    // and `pad axis NAME VALUE`, by the names a browser gives a controller's parts, and
    // `pad here` and `pad gone` as one is plugged in and taken out.

    /** How many controllers are plugged in. */
    private static int controllers() {
        int count = 0;
        for (int id : InputDevice.getDeviceIds()) {
            InputDevice device = InputDevice.getDevice(id);
            if (device != null && !device.isVirtual()
                    && (device.getSources() & InputDevice.SOURCE_GAMEPAD) == InputDevice.SOURCE_GAMEPAD) {
                count++;
            }
        }
        return count;
    }

    /** A controller is plugged in, and what it says it is (`pad kind`): its maker's number,
     *  in hex, and its name, by which the game knows a PlayStation's from an Xbox's and
     *  draws its buttons as they are. */
    private void padHere() {
        say("pad here");
        for (int id : InputDevice.getDeviceIds()) {
            InputDevice device = InputDevice.getDevice(id);
            if (device != null && !device.isVirtual()
                    && (device.getSources() & InputDevice.SOURCE_GAMEPAD) == InputDevice.SOURCE_GAMEPAD) {
                say("pad kind " + String.format("%04x", device.getVendorId()) + " " + device.getName());
                return;
            }
        }
    }

    @Override
    public void onInputDeviceAdded(int id) {
        if (controllers() > 0) padHere();
    }

    @Override
    public void onInputDeviceRemoved(int id) {
        if (controllers() == 0) say("pad gone");
    }

    @Override
    public void onInputDeviceChanged(int id) {}

    /** A controller's button, by a browser's name for it, or nothing if the key is not one. */
    private static String button(int key) {
        switch (key) {
            case KeyEvent.KEYCODE_BUTTON_A: return "south";
            case KeyEvent.KEYCODE_BUTTON_B: return "east";
            case KeyEvent.KEYCODE_BUTTON_X: return "west";
            case KeyEvent.KEYCODE_BUTTON_Y: return "north";
            case KeyEvent.KEYCODE_BUTTON_L1: return "l1";
            case KeyEvent.KEYCODE_BUTTON_R1: return "r1";
            case KeyEvent.KEYCODE_BUTTON_THUMBL: return "l3";
            case KeyEvent.KEYCODE_BUTTON_THUMBR: return "r3";
            case KeyEvent.KEYCODE_BUTTON_START: return "start";
            case KeyEvent.KEYCODE_BUTTON_SELECT: return "select";
            default: return null;
        }
    }

    /** A key: a controller's button, or Back, which is what Escape is on a keyboard. */
    private boolean heardKey(KeyEvent event) {
        boolean down = event.getAction() == KeyEvent.ACTION_DOWN;
        if (down && event.getRepeatCount() > 0) return button(event.getKeyCode()) != null;
        String button = button(event.getKeyCode());
        if (button != null) {
            say("pad button " + button + (down ? " 1" : " 0"));
            return true;
        }
        if (event.getKeyCode() == KeyEvent.KEYCODE_BACK) {
            say("key Escape " + (down ? "1" : "0"));
            return true;
        }
        return false;
    }

    /** What each of a controller's sticks, triggers and d-pad was last told as. */
    private final float[] told = new float[10];
    private static final String[] PARTS = {
        "axis left_x", "axis left_y", "axis right_x", "axis right_y", "button l2", "button r2",
        "button left", "button right", "button up", "button down",
    };

    /** A controller's sticks, triggers and d-pad, which come as one event for them all. */
    private boolean heardSticks(MotionEvent event) {
        if ((event.getSource() & InputDevice.SOURCE_JOYSTICK) != InputDevice.SOURCE_JOYSTICK
                || event.getAction() != MotionEvent.ACTION_MOVE) {
            return false;
        }
        float hatX = event.getAxisValue(MotionEvent.AXIS_HAT_X), hatY = event.getAxisValue(MotionEvent.AXIS_HAT_Y);
        float[] now = {
            event.getAxisValue(MotionEvent.AXIS_X), event.getAxisValue(MotionEvent.AXIS_Y),
            event.getAxisValue(MotionEvent.AXIS_Z), event.getAxisValue(MotionEvent.AXIS_RZ),
            // A trigger is one of two axes, by who made the controller.
            Math.max(event.getAxisValue(MotionEvent.AXIS_LTRIGGER), event.getAxisValue(MotionEvent.AXIS_BRAKE)),
            Math.max(event.getAxisValue(MotionEvent.AXIS_RTRIGGER), event.getAxisValue(MotionEvent.AXIS_GAS)),
            hatX < -0.5f ? 1 : 0, hatX > 0.5f ? 1 : 0, hatY < -0.5f ? 1 : 0, hatY > 0.5f ? 1 : 0,
        };
        for (int part = 0; part < now.length; part++) {
            if (Math.abs(now[part] - told[part]) >= 0.01f || (now[part] == 0 && told[part] != 0)) {
                told[part] = now[part];
                say("pad " + PARTS[part] + " " + now[part]);
            }
        }
        return true;
    }

    /** Runs a script in the pages. */
    public void overlayRun(final String script) {
        runOnUiThread(() -> {
            if (pages != null) pages.evaluateJavascript(script, null);
        });
    }

    /** What the pages have said since this was last called, a line each, or nothing. */
    public String overlayTake() {
        synchronized (said) {
            if (said.isEmpty()) return null;
            String all = String.join("\n", said);
            said.clear();
            return all;
        }
    }

    // ---- The site.

    /**
     * Fetches a file from the game's site: all of it, or, with `to` past `from`, those bytes of
     * it, first to one past last. Nothing if it could not be had; `fetchError` then says why.
     */
    public byte[] fetch(String address, long from, long to) {
        String why = "";
        for (int attempt = 0; attempt < 3; attempt++) {
            HttpURLConnection site = null;
            try {
                if (attempt > 0) Thread.sleep(2000);
                site = (HttpURLConnection) new URL(address).openConnection();
                site.setConnectTimeout(15000);
                site.setReadTimeout(60000);
                if (to > from) {
                    site.setRequestProperty("Range", "bytes=" + from + "-" + (to - 1));
                    site.setRequestProperty("Accept-Encoding", "identity");
                }
                int answer = site.getResponseCode();
                if (answer == 200 || answer == 206) return read(site.getInputStream());
                why = address + ": the site answered " + answer;
                // Asked again only for what may pass.
                if (answer < 500 && answer != 408 && answer != 429) break;
            } catch (IOException error) {
                why = address + ": " + error.getMessage();
            } catch (InterruptedException error) {
                break;
            } finally {
                if (site != null) site.disconnect();
            }
        }
        fetchError = why;
        return null;
    }

    /** Fetches a whole big file from the site into a file. Whether it came. */
    public boolean download(String address, String to) {
        HttpURLConnection site = null;
        try {
            site = (HttpURLConnection) new URL(address).openConnection();
            site.setConnectTimeout(15000);
            site.setReadTimeout(60000);
            site.setRequestProperty("Accept-Encoding", "identity");
            int answer = site.getResponseCode();
            if (answer != 200) {
                fetchError = address + ": the site answered " + answer;
                return false;
            }
            try (InputStream in = site.getInputStream(); OutputStream out = new FileOutputStream(to)) {
                pour(in, out);
            }
            return true;
        } catch (IOException error) {
            fetchError = address + ": " + error.getMessage();
            return false;
        } finally {
            if (site != null) site.disconnect();
        }
    }

    /** Why the last fetch failed. */
    public String fetchError() {
        return fetchError;
    }
}
