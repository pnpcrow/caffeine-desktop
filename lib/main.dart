import 'dart:async';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';

import 'settings_page.dart';
import 'src/rust/api.dart' as core;
import 'src/rust/frb_generated.dart';
import 'theme.dart';
import 'tray.dart';

Future<void> main(List<String> args) async {
  WidgetsFlutterBinding.ensureInitialized();
  await RustLib.init();
  await windowManager.ensureInitialized();

  // True frameless. TitleBarStyle.hidden keeps WS_THICKFRAME, which reserves
  // an invisible 8px non-client resize border on every side: it reads as a
  // transparent halo around the UI, and the DWM's own corner rounding then
  // doubles up against the shell's larger radius. Frameless makes the client
  // area cover the whole window (single curve); edge resizing is restored by
  // DragToResizeArea in the app widget below. Note: WindowOptions must not
  // carry titleBarStyle — setTitleBarStyle() resets the frameless flag.
  await windowManager.setAsFrameless();

  // Startup-folder entries launch with --background: start in the tray
  // instead of popping the settings window over the login session. The
  // runner forwards the command line to this entrypoint (argv[0] skipped).
  final backgroundLaunch =
      args.any((a) => a == '--background' || a == '--minimized');

  // Single instance: the loser focuses the winner's window and exits.
  if (!await core.ensureSingleInstance()) {
    if (!backgroundLaunch) {
      await core.focusExistingWindow();
    }
    exit(0);
  }

  await core.initCore();
  final settings = await core.getSettings();

  const options = WindowOptions(
    size: Size(480, 700),
    minimumSize: Size(440, 620),
    center: true,
    backgroundColor: Colors.transparent,
    skipTaskbar: false,
    title: 'Caffeine Desktop',
  );
  await windowManager.waitUntilReadyToShow(options, () async {
    await windowManager.setPreventClose(true);
  });

  await trayController.init();
  await trayController.refresh(await core.getStatus());

  runApp(const CaffeineApp());

  // Show only after the first frame is built: the runner no longer
  // auto-shows on first frame (that would defeat settings.startMinimized),
  // and showing before the first frame flashes an empty window.
  WidgetsBinding.instance.addPostFrameCallback((_) {
    if (!settings.startMinimized && !backgroundLaunch) {
      windowManager.show();
    }
  });
}

class CaffeineApp extends StatelessWidget {
  const CaffeineApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      debugShowCheckedModeBanner: false,
      title: 'Caffeine Desktop',
      theme: AppTheme.dark(),
      // DragToResizeArea: the frameless window has no native resize borders,
      // so 8px drag strips (edges + corners) are overlaid on the shell. The
      // top strip also keeps the double-click-to-maximize gesture.
      home: const DragToResizeArea(child: SettingsPage()),
    );
  }
}
