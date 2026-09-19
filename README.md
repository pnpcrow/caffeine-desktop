# Caffeine Desktop

Windows 11+ 네이티브 앱. 디스플레이가 꺼지지 않도록(잠자기 방지) 활성 상태를
유지하면서, 필요할 때는 모니터를 검은색으로 가리는 **캡처 방지 블랙아웃**을 제공한다.

핵심 보장: **가림막은 사람 눈에만 보이고, 스크린샷·영상 캡처·AI 비전(Computer Use)에는
절대 찍히지 않는다.** 가림 중에도 AI 에이전트의 마우스/키보드 입력은 그대로 아래 앱에
전달되므로 에이전트 작업이 방해받지 않는다.

- GUI: **Flutter (Windows desktop)**, 설정창 + 시스템 트레이
- 핵심 로직: **Rust 크레이트 (`rust/`)** — Win32 직접 호출, `flutter_rust_bridge` v2
  (native-assets 백엔드)로 연결. Python/Node 런타임 불필요.

## 아키텍처

```
Flutter (lib/) ──FRB──> Rust core (rust/src/)
  설정창(UI)                설정 JSON (%APPDATA%)
  트레이 아이콘/메뉴        절전 방지 (SetThreadExecutionState)
                            블랙아웃 오버레이 (순수 Win32 창, 캡처 제외+클릭스루)
                            상태 OSD (9방향 작은 아이콘, 캡처 제외+클릭스루)
                            커서 찾기 (흔들기 감지 → 확대 커서+파형, 방향 화살표)
                            유휴 감지 (GetLastInputInfo) + 자동 가림
                            전역 후크 + 흔들기 해제 판정
```

설계 원칙 (이전 Tauri 구현의 실패에서 얻은 교훈):

1. **UI 스레드를 절대 블로킹하지 않는다.** 모든 Rust 호출은 FRB 워커 풀에서 실행되고,
   상태 변경은 `Stream<Status>` 푸시로 전달된다. 메인 스레드에서 응답을 기다리는
   호출(`recv`/`Wait` 계열)은 금지.
2. **오버레이는 GUI 프레임워크를 거치지 않는다.** Rust가 Win32 창을 직접 만들어
   `WDA_EXCLUDEFROMCAPTURE` + 클릭스루를 적용한다. 임베딩 투명 합성 문제를 원천 차단.
3. **테마는 완전히 명시한다.** 모든 텍스트 스타일에 장식을 끄고(`decoration: none`),
   버튼/팝업/다이얼로그 테마까지 고정한다. `Switch` 등 Material 위젯은 반드시
   `Scaffold`(= Material 조상) 아래에 둔다. 조상 없는 Material 위젯은 릴리스에서
   깨진 렌더링을 만든다.
4. **`.ps1`은 UTF-8 BOM으로 저장한다.** Windows PowerShell 5.1은 BOM 없는 UTF-8의
   한글 바이트를 오독해서 제어 흐름이 조용히 깨진다(단계 스킵, 인자 손실).

## 기능

| 요구사항 | 구현 |
|---|---|
| 1. 시스템 트레이 상주 + 메뉴 | `lib/tray.dart` — 절전 방지 체크, 지금 화면 가리기, 자동 가리기 체크, 마우스 커서 찾기 체크, 화면 상태 표시(OSD) 체크, 설정 열기, 종료. 좌클릭 = 설정 열기 |
| 2a. 화면 꺼짐 방지 | `rust/src/awake.rs` — `SetThreadExecutionState` + 30초 재주장 |
| 2b. 검은색 차단 + 해제 | `rust/src/overlay.rs` + `unlock.rs` — 모니터별 Win32 전체화면 창, 전역 LL 후크, 흔들기 패턴 감지, 가림 중 화면 중앙에 회색 해제 방법 힌트(설정 기준, 캡처 제외) |
| 3. 무조작 시 자동 차단 (토글) | `rust/src/idle.rs` + `manager.rs` — 1초 폴링, 기동 후 60초 그레이스 |
| 4. 설정 (대기 시간, 해제 방법) | 크롬리스 설정창 — 1~30분, 키보드(아무 키/ESC/스페이스/엔터), 마우스(사용 안 함/움직임/흔들기/클릭) |
| Windows 시작 시 자동 실행 | 설정 화면 토글(`rust/src/autostart.rs`) + 인스톨러 옵션 — 두 경로 모두 같은 시작 폴더 바로가기(`Caffeine Desktop.lnk`)를 관리하고 `--background` 인자로 실행되어 설정창 없이 트레이로 시작. 일반 실행(아이콘 더블클릭)은 기존대로 창 표시 |
| 화면 상태 OSD | `rust/src/osd.rs` — 절전 방지(커피잔)·자동 가림(가린 눈) 아주 작은 아이콘, 9방향 위치 선택(가운데 왼쪽/오른쪽은 세로, 가운데는 십자 에임), 설정에서 켜기/끄기 · 캡처 제외+클릭스루, 가림 중에는 숨김 |
| 마우스 커서 찾기 (0.3.0, 0.3.1 개선) | `rust/src/cursor_find.rs` — 마우스를 좌우로 빠르게 흔들면(550ms 내 방향전환 3회+220px) 선택한 이펙트로 커서 위치를 알려 줌: 커서 확대(별도 토글), 원형 파형(별도 토글), 커서가 없는 모니터 중앙의 모니터 높이 비례 큰 화살표(8방향, 별도 토글). 0.3.1: 확대 재생 중 실제 커서를 투명 커서로 교체해 "커서 두 개" 현상 제거(종료 시 사용자 스킴 복원), 흔드는 속도에 따라 커서·파형·글로우 전체가 기준 크기의 0.5–2.0배로 실시간 호흡(정지 시 잦아듦), 확대 화살표가 글로우에 물들던 합성 순서 버그 수정(순백 채움·검은 외곽선) + 평탄 스트로크 파형·모니터 화살표 불투명도 상향으로 선명도 개선. 기능이 켜져 있으면 OSD에 포인터 아이콘 상시 표시(커피잔·눈 아이콘과 같은 시맨틱). 가림 중에는 흔들기가 해제 제스처로 동작하므로 상호배제 |
| 캡처 방지 | `WDA_EXCLUDEFROMCAPTURE` — `Win+Shift+S`로 검증 (눈엔 검게, 결과엔 바탕 그대로) |
| Computer Use 호환 | `WS_EX_TRANSPARENT \| WS_EX_LAYERED` + 입력 무시 — 에이전트 입력 그대로 통과 |

### AI 에이전트 운용 권장 설정

- 해제: **특정 키(ESC 등) + 흔들기/사용 안 함**. `움직이기`/`클릭`은 에이전트의 첫
  입력에 풀리므로 작업 중 비권장. 흔들기(2초 내 방향전환 3회+450px)는 합성 직선
  경로와 겹치지 않는다(`cargo test`로 검증).
- 합성 입력도 `GetLastInputInfo`를 갱신하므로 작업 중 자동 가림이 발동하지 않는다.

## 필요 조건 (빌드 머신)

- Windows 10 2004+ / Windows 11 (실행 타깃 Windows 11+)
- Flutter SDK 3.x (`FLUTTER_ROOT` 또는 PATH, 본 저장소는 `C:\Develop\flutter` 사용)
- Rust stable (`rustup`, 후크·빌드용) + VS Build Tools C++ 워크로드 (CMake/Ninja 포함)
- `cargo install flutter_rust_bridge_codegen --version "^2"` (브리지 재생성 시에만)
- Inno Setup 6+ (인스톨러용), Node 불필요, WebView2 불필요

## 빌드·실행

```powershell
.\scripts\build.cmd            # 전체: 앱 빌드 -> Inno 인스톨러
.\scripts\build.cmd -NoBundle  # 앱까지만 (빠른 확인)
.\scripts\build.cmd -RegenBridge  # rust/src/api.rs 변경 후 바인딩 재생성 포함
.\scripts\dev.cmd              # flutter run (콘솔 로그)
```

산출물: `build\windows\x64\runner\Release\caffeine_desktop.exe` (+ 엔진/플러그인 DLL),
`installer\Output\caffeine-desktop-setup-<버전>.exe` (Inno, 한국어, VC++ redist同梱 검사).

프로세스 표시 이름: exe 파일명은 `caffeine_desktop.exe`를 유지하되, 작업 관리자 등에서는
버전 리소스의 `FileDescription`(Runner.rc) 기준으로 **Caffeine Desktop**으로 표시된다.

버전 규칙: Rust 크레이트 = Dart `flutter_rust_bridge` = codegen의 **major.minor 일치 필수**
(현재 2.13). 어기면 codegen이 거부한다.

## 검증 상태

- 자동: `cargo test` 9종 (흔들기 판정·키 매트릭스·이동 임계·설정 round-trip·FFI 레이아웃·
  실 오버레이 생성/파괴+지오메트리), `flutter analyze` clean,
  `tools/test_autofire.ps1` (무입력 자동발화→오버레이 HWND/affinity 로그/픽셀 캡처제외),
  인스톨러 무음 설치/제거.
- 수동 확인 체크리스트 (2분): 트레이 우클릭 5종, 토글 2회(응답 유지), [지금 화면 가리기] →
  전 모니터 검게 → ESC/흔들기 해제, 가림 중 `Win+Shift+S` 캡처 제외 확인.

## 문제 해결

- **`.ps1`이 이상 동작(단계 스킵 등)** → 파일이 UTF-8 BOM인지 확인.
- **FRB 버전 불일치** → codegen ↔ `flutter_rust_bridge` ↔ Rust 크레이트 2.13 고정.
- **`No Material widget found`** → Material 위젯은 반드시 `Scaffold` 아래에.
- **가림막이 캡처에 찍힌다** → Win10 2004 미만 또는 affinity 실패. `%LOCALAPPDATA%\
  caffeine-desktop\core.log`에서 `capture_excluded=` 확인.
- **진단 로그**: Rust 로그 `%LOCALAPPDATA%\
  caffeine-desktop\core.log`,
  설정 `%APPDATA%\caffeine-desktop\settings.json`.

## 라이선스

MIT (추후 명시).
