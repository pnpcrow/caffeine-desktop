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
| 마우스 커서 찾기 | `rust/src/cursor_find.rs` — 빠른 좌우 흔들기로 커서 확대·원형 파형·다른 모니터 방향 화살표. 고주사율 마우스의 작은 이동도 누적 판정하며, 시간 기반 크기 보간과 파형별 반경 유지, 부드러운 진입·종료를 적용. |
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

- 자동: `cargo test` 42종 (실제 창의 최상위 순서 복구·메시지 응답·커서 복원, 알파 합성·잔상 제거, 고폴링 흔들기, 프레임률 독립 보간 포함), `flutter test` 3종 (최소 창·130% 글꼴 포함), `flutter analyze`, Windows 릴리스 빌드.
- 수동 확인 체크리스트 (2분): 트레이 우클릭 5종, 토글 2회(응답 유지), [지금 화면 가리기] →
  전 모니터 검게 → ESC/흔들기 해제, 가림 중 `Win+Shift+S` 캡처 제외 확인.

## 0.4.0 렌더링과 디자인

- OSD는 표시 중 500ms 간격으로 일반 데스크톱의 최상위 순서를 재확인한다. 디스플레이·DPI·DWM 합성 변경 시 다시 그리며, 가림 중/OSD 비활성 상태에서는 타이머를 중지한다. 포커스를 가져오지 않는다.
- GDI 작업이 끝난 뒤 DIB 알파값을 계산한다. 커서 효과 창도 Windows 메시지를 처리하며, 프레임마다 투명 버퍼를 지우고 premultiplied ARGB로 합성한다. 위치와 픽셀은 UpdateLayeredWindow에서 함께 갱신한다.
- 애니메이션은 단조 고해상도 시간으로 계산하고 DwmFlush로 합성 시점에 맞춘다(최대 약 125fps). 커서 위치는 래스터화 직후 최신 좌표를 읽고, 방향 화살표는 30fps로 제한한다. 커서 확대는 원래 크기에서 부드럽게 커졌다 돌아오며, 파형은 생성 시 반경을 유지하고 잔광은 따로 페이드한다.
- 커서 대체 프레임이 실제로 전달된 다음에만 시스템 커서를 숨긴다. 표시 실패·종료·취소 시 저장해 둔 실제 커서 모양을 복원한다(실패 시 저장된 사용자 스킴으로 복구).
- 첨부 캐릭터의 크림·모카·청회색을 사용한 밝은 테마와 커피 일러스트. 앱/트레이/설치 아이콘은 `gen-bot-face` 스킬 지침으로 생성한 얼굴을 사용한다.

일반 데스크톱의 topmost 창을 대상으로 한다. 보안 데스크톱(UAC/잠금), Windows가 별도 관리하는 시스템 레이어, 독점 전체화면 위 표시까지 보장하지 않는다. 특정 Windows 업데이트가 원인이라는 점은 확정하지 않았으며, 해당 환경에서의 장시간 고속 이동·다중 모니터 실사용 확인은 별도로 필요하다.

구현 근거: [DIB와 GDI 동기화](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/nf-wingdi-createdibsection), [최상위 순서](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos), [DWM 프레임 동기화](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmflush).

이미지 원본과 생성 프롬프트: `assets/artwork.json`, `assets/illustrations/coffee-companion.png`, `assets/icons/icon-source.png`. 기본 내장 이미지 도구로 제작했으며 외부 API 키는 사용하지 않았다. 아이콘 크기/포맷 재생성은 `python tools/make_icons.py` (개발 시 Pillow 필요). `assets/icons`와 Windows runner 리소스를 함께 갱신한다.

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
