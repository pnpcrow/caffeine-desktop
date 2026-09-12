import 'dart:io';

import 'package:flutter/services.dart';
import 'package:tray_manager/tray_manager.dart';
import 'package:window_manager/window_manager.dart';

import 'src/rust/api.dart' as core;
import 'src/rust/manager.dart' show Status;

/// System-tray icon + menu. Everything here is async MethodChannel traffic:
/// no call ever blocks the Flutter UI thread (this is what killed Tauri).
class TrayController with TrayListener {
  Future<void> init() async {
    // tray_manager needs a real file path: materialize the bundled icon.
    final bytes = await rootBundle.load('assets/icons/icon.ico');
    final file = File(
        '${Directory.systemTemp.path}\\caffeine-desktop-tray.ico');
    await file.writeAsBytes(
        bytes.buffer.asUint8List(bytes.offsetInBytes, bytes.lengthInBytes));
    await trayManager.setIcon(file.path);
    await trayManager.setToolTip('Caffeine Desktop — 디스플레이 절전 방지');
    trayManager.addListener(this);
  }

  /// Rebuild the popup menu from current status. Safe to call from anywhere:
  /// pure async channel call, never blocks the UI thread.
  Future<void> refresh(Status s) async {
    await trayManager.setContextMenu(Menu(items: [
      MenuItem.checkbox(
        key: 'awake',
        label: '절전 방지',
        checked: s.awake,
        onClick: (_) => core.setAwake(on_: !s.awake),
      ),
      MenuItem(
        key: 'blackout',
        label: '지금 화면 가리기',
        onClick: (_) => core.blackoutNow(),
      ),
      MenuItem.checkbox(
        key: 'auto',
        label: '자동 화면 가리기',
        checked: s.autoBlackout,
        onClick: (_) => core.setAutoBlackout(on_: !s.autoBlackout),
      ),
      MenuItem.separator(),
      MenuItem(
        key: 'settings',
        label: '설정 열기',
        onClick: (_) => showSettings(),
      ),
      MenuItem.separator(),
      MenuItem(
        key: 'quit',
        label: '종료',
        onClick: (_) => quitApp(),
      ),
    ]));
  }

  @override
  void onTrayIconMouseDown() {
    // Windows delivers the left-button release as onTrayIconMouseDown
    // (onTrayIconMouseUp is never fired on this platform).
    showSettings();
  }

  @override
  void onTrayIconRightMouseDown() {
    // Windows delivers the right-button release as onTrayIconRightMouseDown,
    // and the menu is NOT shown automatically: the app must pop it up.
    // bringAppToFront is the classic SetForegroundWindow-before-TrackPopupMenu
    // fix; without it the menu can misbehave (won't dismiss on outside click).
    // ignore: deprecated_member_use
    trayManager.popUpContextMenu(bringAppToFront: true);
  }
}

final trayController = TrayController();

Future<void> showSettings() async {
  // isVisible() is also true while minimized (iconic), so restore explicitly:
  // focus() alone would not bring a minimized window back.
  if (await windowManager.isMinimized()) {
    await windowManager.restore();
  }
  if (!await windowManager.isVisible()) {
    await windowManager.show();
  }
  await windowManager.focus();
}

Future<void> quitApp() async {
  await core.prepareQuit();
  await trayManager.destroy();
  exit(0);
}
