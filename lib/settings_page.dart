import 'dart:async';

import 'package:flutter/material.dart';
import 'package:window_manager/window_manager.dart';

import 'src/rust/api.dart' as core;
import 'src/rust/manager.dart' show Status;
import 'src/rust/settings.dart'
    show OsdPosition, Settings, UnlockKey, UnlockMouse;
import 'theme.dart';
import 'tray.dart';

const _timeOptions = <int, String>{
  60: '1분',
  120: '2분',
  180: '3분',
  300: '5분',
  600: '10분',
  900: '15분',
  1800: '30분',
};

const _osdGrid = [
  [OsdPosition.topLeft, OsdPosition.topCenter, OsdPosition.topRight],
  [OsdPosition.middleLeft, OsdPosition.center, OsdPosition.middleRight],
  [OsdPosition.bottomLeft, OsdPosition.bottomCenter, OsdPosition.bottomRight],
];

String _posLabel(OsdPosition p) => switch (p) {
      OsdPosition.topLeft => '왼쪽 위',
      OsdPosition.topCenter => '가운데 위',
      OsdPosition.topRight => '오른쪽 위',
      OsdPosition.middleLeft => '가운데 왼쪽',
      OsdPosition.center => '가운데',
      OsdPosition.middleRight => '가운데 오른쪽',
      OsdPosition.bottomLeft => '아래 왼쪽',
      OsdPosition.bottomCenter => '아래 가운데',
      OsdPosition.bottomRight => '아래 오른쪽',
    };

String _keyLabel(UnlockKey k) => switch (k) {
      UnlockKey.any => '아무 키나',
      UnlockKey.esc => 'ESC 키만',
      UnlockKey.space => '스페이스바만',
      UnlockKey.enter => '엔터 키만',
    };

String _mouseLabel(UnlockMouse m) => switch (m) {
      UnlockMouse.off => '사용 안 함 (키보드로만 해제)',
      UnlockMouse.shake => '마우스 흔들기',
      UnlockMouse.move => '마우스 움직이기',
      UnlockMouse.click => '마우스 클릭',
    };

class SettingsPage extends StatefulWidget {
  const SettingsPage({super.key});

  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> with WindowListener {
  Settings? _settings;
  Status? _status;
  bool _autostart = false;
  StreamSubscription<Status>? _sub;

  @override
  void initState() {
    super.initState();
    windowManager.addListener(this);
    _reload();
    _sub = core.watchEvents().listen((status) async {
      if (!mounted) return;
      setState(() => _status = status);
      await trayController.refresh(status);
    });
  }

  @override
  void dispose() {
    windowManager.removeListener(this);
    _sub?.cancel();
    super.dispose();
  }

  @override
  void onWindowClose() async {
    // Frameless ✕ means "minimize to tray", quit lives in the tray menu.
    await windowManager.hide();
  }

  Future<void> _reload() async {
    final s = await core.getSettings();
    final st = await core.getStatus();
    final autostart = await core.autostartEnabled();
    if (!mounted) return;
    setState(() {
      _settings = s;
      _status = st;
      _autostart = autostart;
    });
    await trayController.refresh(st);
  }

  Future<void> _push() async {
    final s = _settings;
    if (s == null) return;
    await core.saveSettings(s: s);
  }

  Future<void> _hide() => windowManager.hide();

  @override
  Widget build(BuildContext context) {
    final settings = _settings;
    final status = _status;
    // Scaffold (=> Material ancestor) is mandatory: Switch, buttons and
    // dropdowns require it, and without it release builds render garbage.
    return Scaffold(
      backgroundColor: Colors.transparent,
      body: Container(
      decoration: AppTheme.shell(),
      child: Column(
        children: [
          _titlebar(),
          Expanded(
            child: settings == null || status == null
                ? const Center(
                    child: Text('불러오는 중…',
                        style: TextStyle(color: AppTheme.muted)))
                : SingleChildScrollView(
                    padding: const EdgeInsets.fromLTRB(16, 4, 16, 0),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        _hero(),
                        const SizedBox(height: 10),
                        _statusCard(status),
                        const SizedBox(height: 10),
                        _awakeCard(settings, status),
                        const SizedBox(height: 10),
                        _autoCard(settings),
                        const SizedBox(height: 10),
                        _cursorFindCard(settings),
                        const SizedBox(height: 10),
                        _osdCard(settings),
                        const SizedBox(height: 10),
                        _autostartCard(),
                        const SizedBox(height: 10),
                        _unlockCard(settings),
                        const SizedBox(height: 12),
                        _footer(),
                        const SizedBox(height: 16),
                      ],
                    ),
                  ),
          ),
        ],
      ),
      ),
    );
  }

  Widget _titlebar() {    return DragToMoveArea(
      child: Container(
        padding: const EdgeInsets.fromLTRB(16, 10, 8, 6),
        child: Row(
          children: [
            Image.asset('assets/icons/icon.png', width: 24, height: 24),
            const SizedBox(width: 9),
            const Text('Caffeine Desktop',
                style: TextStyle(
                    color: AppTheme.cream,
                    fontSize: 13,
                    fontWeight: FontWeight.w700)),
            const Spacer(),
            _tbtn('–', _hide),
            _tbtn('✕', _hide, danger: true),
          ],
        ),
      ),
    );
  }

  Widget _tbtn(String glyph, VoidCallback onTap, {bool danger = false}) {
    return SizedBox(
      width: 30,
      height: 26,
      child: TextButton(
        style: TextButton.styleFrom(
          padding: EdgeInsets.zero,
          shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(8)),
        ),
        onPressed: onTap,
        child: Text(glyph,
            style: TextStyle(
                color: danger ? const Color(0xFFF3B8B1) : AppTheme.muted,
                fontSize: 14)),
      ),
    );
  }

  Widget _hero() {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 4, vertical: 4),
      child: Row(
        children: [
          Image.asset('assets/icons/icon.png', width: 104, height: 104),
          const SizedBox(width: 16),
          const Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text('깨어 있는 화면',
                    style: TextStyle(
                        color: AppTheme.cream,
                        fontSize: 21,
                        fontWeight: FontWeight.w800)),
                SizedBox(height: 5),
                Text('절전 방지 + 캡처에 보이지 않는 화면 가림',
                    style: TextStyle(color: AppTheme.muted, fontSize: 12.5)),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _card(Widget child) {
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: AppTheme.cardDeco(),
      child: child,
    );
  }

  Widget _statusCard(Status s) {
    return _card(Row(
      children: [
        _pill(s.awake ? '절전 방지 켜짐' : '절전 방지 꺼짐', s.awake),
        const SizedBox(width: 8),
        _pill(s.blackout ? '화면 가림 중' : '화면 정상',
            !s.blackout ? true : null,
            warn: s.blackout),
        const Spacer(),
        Text('v${s.version}',
            style: const TextStyle(color: AppTheme.muted, fontSize: 11)),
      ],
    ));
  }

  Widget _pill(String text, bool? on, {bool warn = false}) {
    final color = warn
        ? AppTheme.accent
        : (on == true ? AppTheme.green : AppTheme.red);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 11, vertical: 6),
      decoration: BoxDecoration(
        color: Colors.black.withValues(alpha: 0.25),
        border: Border.all(color: color.withValues(alpha: 0.5)),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
              width: 8,
              height: 8,
              decoration:
                  BoxDecoration(color: color, shape: BoxShape.circle)),
          const SizedBox(width: 7),
          Text(text,
              style: const TextStyle(
                  color: AppTheme.cream,
                  fontSize: 12,
                  fontWeight: FontWeight.w600)),
        ],
      ),
    );
  }

  Widget _awakeCard(Settings s, Status st) {
    return _card(Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            const Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('디스플레이 절전 방지',
                      style: TextStyle(
                          color: AppTheme.cream,
                          fontSize: 14,
                          fontWeight: FontWeight.w700)),
                  SizedBox(height: 4),
                  Text('화면 꺼짐·잠자기·화면 보호기를 막고 활성 상태를 유지합니다.',
                      style: TextStyle(color: AppTheme.muted, fontSize: 12)),
                ],
              ),
            ),
            Switch(
              value: s.awakeEnabled,
              activeThumbColor: AppTheme.accent,
              onChanged: (v) async {
                setState(() => _settings = _copy(s, awakeEnabled: v));
                await core.setAwake(on_: v);
              },
            ),
          ],
        ),
        const SizedBox(height: 11),
        Row(
          children: [
            Expanded(
              child: ElevatedButton(
                style: ElevatedButton.styleFrom(
                  backgroundColor: AppTheme.accent,
                  foregroundColor: const Color(0xFF241305),
                  padding: const EdgeInsets.symmetric(vertical: 12),
                  shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(10)),
                ),
                onPressed: st.blackout
                    ? null
                    : () async {
                        await core.blackoutNow();
                      },
                child: const Text('지금 화면 가리기',
                    style: TextStyle(fontWeight: FontWeight.w700)),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: OutlinedButton(
                style: OutlinedButton.styleFrom(
                  foregroundColor: AppTheme.cream,
                  side: const BorderSide(color: AppTheme.cardBorder),
                  padding: const EdgeInsets.symmetric(vertical: 12),
                  shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(10)),
                ),
                onPressed: st.blackout
                    ? () async {
                        await core.clearBlackout();
                      }
                    : null,
                child: const Text('가림 해제'),
              ),
            ),
          ],
        ),
      ],
    ));
  }

  Widget _autoCard(Settings s) {
    return _card(Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            const Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('자동 화면 가리기',
                      style: TextStyle(
                          color: AppTheme.cream,
                          fontSize: 14,
                          fontWeight: FontWeight.w700)),
                  SizedBox(height: 4),
                  Text('조작이 없으면 일정 시간 후 자동으로 화면을 가립니다.',
                      style: TextStyle(color: AppTheme.muted, fontSize: 12)),
                ],
              ),
            ),
            Switch(
              value: s.autoBlackoutEnabled,
              activeThumbColor: AppTheme.accent,
              onChanged: (v) async {
                setState(() => _settings = _copy(s, autoBlackoutEnabled: v));
                await core.setAutoBlackout(on_: v);
              },
            ),
          ],
        ),
        const SizedBox(height: 11),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            const Text('가리기까지 대기 시간',
                style: TextStyle(color: AppTheme.muted, fontSize: 12.5)),
            _dropdown<int>(
              value: s.autoBlackoutSecs.toInt(),
              items: _timeOptions,
              onChanged: (v) async {
                if (v == null) return;
                setState(
                    () => _settings = _copy(s, autoBlackoutSecs: v));
                await _push();
              },
            ),
          ],
        ),
      ],
    ));
  }

  Widget _osdCard(Settings s) {
    final enabled = s.osdEnabled;
    return _card(Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            const Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('화면 상태 표시 (OSD)',
                      style: TextStyle(
                          color: AppTheme.cream,
                          fontSize: 14,
                          fontWeight: FontWeight.w700)),
                  SizedBox(height: 4),
                  Text(
                      '절전 방지·자동 가림이 켜져 있으면 아주 작은 아이콘으로 알려줍니다. '
                      '캡처에는 보이지 않고 화면 사용도 막지 않습니다.',
                      style: TextStyle(color: AppTheme.muted, fontSize: 12)),
                ],
              ),
            ),
            Switch(
              value: enabled,
              activeThumbColor: AppTheme.accent,
              onChanged: (v) async {
                setState(() => _settings = _copy(s, osdEnabled: v));
                await _push();
              },
            ),
          ],
        ),
        const SizedBox(height: 10),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            const Text('표시 위치',
                style: TextStyle(color: AppTheme.muted, fontSize: 12.5)),
            Text(_posLabel(s.osdPosition),
                style: const TextStyle(
                    color: AppTheme.accent,
                    fontSize: 12,
                    fontWeight: FontWeight.w700)),
          ],
        ),
        const SizedBox(height: 8),
        // Grid is inert while the OSD is off.
        Opacity(
          opacity: enabled ? 1 : 0.45,
          child: IgnorePointer(
            ignoring: !enabled,
            child: Column(
              children: [
                for (final row in _osdGrid) ...[
                  Row(
                    children: [
                      for (final p in row) ...[
                        Expanded(child: _posCell(s, p)),
                        if (p != row.last) const SizedBox(width: 8),
                      ],
                    ],
                  ),
                  if (row != _osdGrid.last) const SizedBox(height: 8),
                ],
              ],
            ),
          ),
        ),
      ],
    ));
  }

  /// One 3x3 selector cell: dots laid out the way the OSD itself will be
  /// (horizontal on top/bottom rows, vertical on the middle sides, a
  /// crosshair at the center), anchored at the cell's own position.
  Widget _posCell(Settings s, OsdPosition p) {
    final selected = s.osdPosition == p;
    final color = selected ? AppTheme.accent : AppTheme.muted;
    final dot = Container(
      width: 4,
      height: 4,
      decoration: BoxDecoration(color: color, shape: BoxShape.circle),
    );
    final Widget glyph = switch (p) {
      OsdPosition.middleLeft ||
      OsdPosition.middleRight =>
        Column(mainAxisSize: MainAxisSize.min, children: [
          dot,
          const SizedBox(height: 3),
          dot,
        ]),
      OsdPosition.center => Icon(Icons.add, size: 15, color: color),
      _ => Row(mainAxisSize: MainAxisSize.min, children: [
          dot,
          const SizedBox(width: 3),
          dot,
        ]),
    };
    final alignment = switch (p) {
      OsdPosition.topLeft => Alignment.topLeft,
      OsdPosition.topCenter => Alignment.topCenter,
      OsdPosition.topRight => Alignment.topRight,
      OsdPosition.middleLeft => Alignment.centerLeft,
      OsdPosition.center => Alignment.center,
      OsdPosition.middleRight => Alignment.centerRight,
      OsdPosition.bottomLeft => Alignment.bottomLeft,
      OsdPosition.bottomCenter => Alignment.bottomCenter,
      OsdPosition.bottomRight => Alignment.bottomRight,
    };
    return Tooltip(
      message: _posLabel(p),
      child: InkWell(
        borderRadius: BorderRadius.circular(8),
        onTap: () async {
          setState(() => _settings = _copy(s, osdPosition: p));
          await _push();
        },
        child: Container(
          height: 38,
          decoration: BoxDecoration(
            color: selected
                ? AppTheme.accent.withValues(alpha: 0.15)
                : Colors.black.withValues(alpha: 0.22),
            border: Border.all(
                color: selected
                    ? AppTheme.accent
                    : AppTheme.cardBorder.withValues(alpha: 0.6)),
            borderRadius: BorderRadius.circular(8),
          ),
          child: Align(
            alignment: alignment,
            widthFactor: 1,
            heightFactor: 1,
            child: Padding(padding: const EdgeInsets.all(6), child: glyph),
          ),
        ),
      ),
    );
  }

  /// Shake-to-find-the-cursor: master toggle + the effect sub-options
  /// (magnify / ripple / multi-monitor arrows), inert while the feature
  /// itself is off.
  Widget _cursorFindCard(Settings s) {
    final enabled = s.cursorFindEnabled;
    Widget subRow(String label, bool value, ValueChanged<bool?> onChanged) {
      return Row(
        children: [
          Expanded(
            child: Text(label,
                style: TextStyle(color: AppTheme.muted, fontSize: 12.5)),
          ),
          Switch(
            value: value,
            activeThumbColor: AppTheme.accent,
            onChanged: onChanged,
          ),
        ],
      );
    }

    return _card(Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            const Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('마우스 커서 찾기',
                      style: TextStyle(
                          color: AppTheme.cream,
                          fontSize: 14,
                          fontWeight: FontWeight.w700)),
                  SizedBox(height: 4),
                  Text(
                      '마우스를 좌우로 빠르게 흔들면 커서 위치를 바로 찾을 수 있게 '
                      '선택한 이펙트를 보여 줍니다.',
                      style: TextStyle(color: AppTheme.muted, fontSize: 12)),
                ],
              ),
            ),
            Switch(
              value: enabled,
              activeThumbColor: AppTheme.accent,
              onChanged: (v) async {
                setState(() => _settings = _copy(s, cursorFindEnabled: v));
                await _push();
              },
            ),
          ],
        ),
        const SizedBox(height: 6),
        Opacity(
          opacity: enabled ? 1 : 0.45,
          child: IgnorePointer(
            ignoring: !enabled,
            child: Column(
              children: [
                subRow('마우스 커서 크게 키우기 (흔든 강도에 비례)', s.cursorFindMagnify,
                    (v) async {
                  setState(() => _settings = _copy(s, cursorFindMagnify: v));
                  await _push();
                }),
                subRow('커서 주변에 원형 파형 이펙트', s.cursorFindRipple, (v) async {
                  setState(() => _settings = _copy(s, cursorFindRipple: v));
                  await _push();
                }),
                subRow('다른 모니터에 방향 화살표 표시', s.cursorFindArrows, (v) async {
                  setState(() => _settings = _copy(s, cursorFindArrows: v));
                  await _push();
                }),
              ],
            ),
          ),
        ),
        const SizedBox(height: 2),
        Text(
            '커서가 없는 모니터 가운데에 커서가 있는 모니터 방향의 큰 화살표를 보여 줍니다.',
            style: TextStyle(
                color: AppTheme.muted.withValues(alpha: enabled ? 1 : 0.45),
                fontSize: 11)),
      ],
    ));
  }

  /// Startup entry toggle. Writes/removes the same "Caffeine Desktop.lnk"
  /// the installer's startup task manages (always with --background, i.e.
  /// boot starts land in the tray without opening this window).
  Widget _autostartCard() {
    return _card(Row(
      children: [
        const Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('시스템 시작 시 자동 실행',
                  style: TextStyle(
                      color: AppTheme.cream,
                      fontSize: 14,
                      fontWeight: FontWeight.w700)),
              SizedBox(height: 4),
              Text(
                  'Windows가 켜지면 Caffeine Desktop이 백그라운드(트레이)로 자동 시작됩니다. '
                  '설정 창은 열리지 않습니다.',
                  style: TextStyle(color: AppTheme.muted, fontSize: 12)),
            ],
          ),
        ),
        Switch(
          value: _autostart,
          activeThumbColor: AppTheme.accent,
          onChanged: (v) => _setAutostart(v),
        ),
      ],
    ));
  }

  Future<void> _setAutostart(bool v) async {
    setState(() => _autostart = v);
    try {
      await core.setAutostart(on_: v);
    } catch (e) {
      // COM/filesystem failure: reflect the unchanged on-disk state.
      if (!mounted) return;
      setState(() => _autostart = !v);
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text('자동 실행 설정 변경에 실패했습니다: $e'),
          backgroundColor: AppTheme.bg2,
        ),
      );
    }
  }

  Widget _unlockCard(Settings s) {
    return _card(Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const Text('가림 해제 방법',
            style: TextStyle(
                color: AppTheme.cream,
                fontSize: 14,
                fontWeight: FontWeight.w700)),
        const SizedBox(height: 4),
        const Text('검은 화면을 해제하는 조건을 설정합니다.',
            style: TextStyle(color: AppTheme.muted, fontSize: 12)),
        const SizedBox(height: 11),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            const Text('키보드',
                style: TextStyle(color: AppTheme.muted, fontSize: 12.5)),
            _dropdown<UnlockKey>(
              value: s.unlockKey,
              items: {for (final k in UnlockKey.values) k: _keyLabel(k)},
              onChanged: (v) async {
                if (v == null) return;
                setState(() => _settings = _copy(s, unlockKey: v));
                await _push();
              },
            ),
          ],
        ),
        const SizedBox(height: 11),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            const Text('마우스',
                style: TextStyle(color: AppTheme.muted, fontSize: 12.5)),
            _dropdown<UnlockMouse>(
              value: s.unlockMouse,
              items: {for (final m in UnlockMouse.values) m: _mouseLabel(m)},
              onChanged: (v) async {
                if (v == null) return;
                setState(() => _settings = _copy(s, unlockMouse: v));
                await _push();
              },
            ),
          ],
        ),
        const SizedBox(height: 11),
        Container(
          padding: const EdgeInsets.all(10),
          decoration: BoxDecoration(
            color: AppTheme.accent.withValues(alpha: 0.08),
            border: Border.all(
                color: AppTheme.accent.withValues(alpha: 0.4),
                style: BorderStyle.solid),
            borderRadius: BorderRadius.circular(10),
          ),
          child: const Text(
            'AI 에이전트(Computer Use) 동작 중에는 키보드 + 흔들기/사용 안 함 조합을 권장합니다. '
            '가림막은 캡처에서 제외되고 입력은 그대로 통과하므로 에이전트는 정상 동작합니다.',
            style: TextStyle(color: AppTheme.muted, fontSize: 11.5, height: 1.6),
          ),
        ),
      ],
    ));
  }

  Widget _dropdown<T>({
    required T value,
    required Map<T, String> items,
    required ValueChanged<T?> onChanged,
  }) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10),
      decoration: BoxDecoration(
        color: AppTheme.bg2,
        border: Border.all(color: AppTheme.cardBorder),
        borderRadius: BorderRadius.circular(9),
      ),
      child: DropdownButton<T>(
        value: value,
        underline: const SizedBox.shrink(),
        dropdownColor: AppTheme.bg2,
        style: const TextStyle(color: AppTheme.cream, fontSize: 12.5),
        items: [
          for (final e in items.entries)
            DropdownMenuItem(value: e.key, child: Text(e.value)),
        ],
        onChanged: onChanged,
      ),
    );
  }

  Widget _footer() {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: OutlinedButton(
                style: OutlinedButton.styleFrom(
                  foregroundColor: AppTheme.cream,
                  side: const BorderSide(color: AppTheme.cardBorder),
                  padding: const EdgeInsets.symmetric(vertical: 11),
                  shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(10)),
                ),
                onPressed: _hide,
                child: const Text('트레이로 최소화'),
              ),
            ),
            const SizedBox(width: 8),
            Expanded(
              child: OutlinedButton(
                style: OutlinedButton.styleFrom(
                  foregroundColor: const Color(0xFFF3B8B1),
                  side: BorderSide(
                      color: AppTheme.red.withValues(alpha: 0.4)),
                  padding: const EdgeInsets.symmetric(vertical: 11),
                  shape: RoundedRectangleBorder(
                      borderRadius: BorderRadius.circular(10)),
                ),
                onPressed: () => quitApp(),
                child: const Text('종료'),
              ),
            ),
          ],
        ),
        const SizedBox(height: 9),
        const Text('가림 동작 중 스크린샷·영상 캡처에는 검은 영역이 찍히지 않습니다.',
            textAlign: TextAlign.center,
            style: TextStyle(color: AppTheme.muted, fontSize: 11)),
      ],
    );
  }

  Settings _copy(
    Settings s, {
    bool? awakeEnabled,
    bool? autoBlackoutEnabled,
    int? autoBlackoutSecs,
    UnlockKey? unlockKey,
    UnlockMouse? unlockMouse,
    bool? osdEnabled,
    OsdPosition? osdPosition,
    bool? cursorFindEnabled,
    bool? cursorFindMagnify,
    bool? cursorFindRipple,
    bool? cursorFindArrows,
  }) {
    return Settings(
      awakeEnabled: awakeEnabled ?? s.awakeEnabled,
      autoBlackoutEnabled: autoBlackoutEnabled ?? s.autoBlackoutEnabled,
      autoBlackoutSecs: BigInt.from(autoBlackoutSecs ?? s.autoBlackoutSecs.toInt()),
      unlockKey: unlockKey ?? s.unlockKey,
      unlockMouse: unlockMouse ?? s.unlockMouse,
      startMinimized: s.startMinimized,
      osdEnabled: osdEnabled ?? s.osdEnabled,
      osdPosition: osdPosition ?? s.osdPosition,
      cursorFindEnabled: cursorFindEnabled ?? s.cursorFindEnabled,
      cursorFindMagnify: cursorFindMagnify ?? s.cursorFindMagnify,
      cursorFindRipple: cursorFindRipple ?? s.cursorFindRipple,
      cursorFindArrows: cursorFindArrows ?? s.cursorFindArrows,
    );
  }
}
