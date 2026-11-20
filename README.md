# belt-drive

로봇 관절용 1단 / 2단 타이밍 벨트 감속기 설계 도구 (Rust → WebAssembly, 정적 웹 앱).
Timing-belt reduction designer for robot joints (Rust → WebAssembly, static web app).

> **교육용 추정치입니다 — 로봇에 맡기기 전에 벨트 제조사 자료와 실제 시험으로 검증하세요.**
> **Educational estimates — validate with the belt maker's data and real tests before trusting a robot with it.**

---

## 한국어

### 무엇인가
준직접구동(QDD) 다리나 손목 같은 로봇 관절에 1단 또는 2단 타이밍 벨트 감속을 쓸 때
"풀리 잇수는? 벨트 길이와 축간 거리는? 벨트 폭은 얼마나? 초기장력은? 강성은 얼마나 되고,
기어박스 대비 백래시 없는 강성을 얼마나 잃는가?"에 답하는 단일 페이지 앱입니다.
모든 계산은 브라우저 안에서만 이루어집니다 (서버, 분석 도구, 외부 요청 없음).

- 벨트 프로파일 프리셋 4종 (GT형 2 mm / 3 mm, HTD형 5 mm, T형 5 mm 스틸 코드) — 모두 "일반값(typical)"이며 특정 제조사 데이터가 아닙니다.
- 단별 구동 / 피동 잇수, 목표 축간 거리, 벨트 폭, 모터 토크·최고속, 출력 피크 토크, 초기장력 계수, 사용 계수 입력.
- 정수 잇수 벨트로 스냅한 실제 축간 거리, 감김각, 물림 잇수, 장력, 필요 폭, 벨트 속도, 스팬 진동수, 출력 강성, 변형, 축 하중과 항목별 / 종합 상태(정상 / 주의 / 불가).
- 감속비 찾기: 목표 총 감속비와 허용 오차로 모든 검사를 통과하는 조합 상위 12개 (행 클릭 시 불러오기).
- 그래프: 벨트 폭에 따른 출력 강성 / 변형, 출력 토크에 따른 T1 / T2 (T2 = 0 이빨 뜀 한계 표시).
- WebGL2 3D 뷰: 이빨 있는 풀리, 플랜지, 축, 모터, 프레임, 이빨 블록 벨트 애니메이션, 장력 색칠, 처짐 / 변형 과장, 축 하중 화살표, 치수 라벨. WebGL2가 없으면 안내 문구만 보이고 수치와 그래프는 그대로 동작합니다.
- 한국어 / 영어 (브라우저 언어 기본), 모바일 지원, 시스템 라이트 / 다크 테마.

### 모델과 가정
교과서 수식만 사용합니다.
- 피치원 지름 d = z·p/π, 단 감속비 z2/z1, 총 감속비는 곱.
- 오픈 벨트 길이 L = 2√(C² − e²) + π(D + d)/2 + (D − d)·asin(e/C), e = (D − d)/2. 목표 C에 가장 가까운 정수 잇수 벨트를 고르고 뉴턴법으로 실제 C를 구합니다.
- 작은 풀리 감김각 θ = π − 2·asin((D − d)/2C), 물림 잇수 = ⌊z·θ/2π⌋ (6 미만 주의, 3 미만 불가).
- 설계 부하: 출력 토크 = max(요구 피크 토크, 모터 토크 × 감속비). 각 단의 구동 풀리 토크로 환산.
- 유효 장력 Te = 2T/d, 원심 장력 Tc = m'·v², 초기장력 Ti = 계수 × (허용 장력/mm × 폭), T1 = Ti + Te/2, T2 = Ti − Te/2 (T2 ≤ 0이면 이빨 뜀 위험 → 불가).
  기본 계수 0.5에서는 Te가 벨트 허용 장력을 넘을 때 T2가 0이 됩니다.
- 필요 폭 b = Te·사용계수 / (허용 장력/mm − m'/mm·v²).
- 벨트 속도 v = π·d1·n/60, 프로파일 최고 속도로부터 모터 회전수 한계.
- 스팬 진동수 f = (1/2L)·√(Ti/m') — 휴대폰 튜너 앱으로 초기장력을 맞출 때 사용.
- 스팬 강성 k = EA/L_span. 이완측이 팽팽한 동안 두 스팬이 함께 작용: K = 2k·r². 2단은 직렬 스프링이며 1단 강성은 2단 감속비의 제곱으로 출력축에 환산됩니다. 변형 = 부하 / K (arcmin).
- 축 하중 = T1과 T2의 벡터 합 √(T1² + T2² − 2·T1·T2·cos θ).

가정: 비신장(inextensible) 기하, 선형 탄성 스팬, 이빨 변형 / 백래시 모델 없음, 일반적인 대표값. 벨트 수명, 아이들러, 서펜타인, 열 효과는 다루지 않습니다.

### 조작
- 왼쪽 드래그: 회전, 오른쪽 드래그 / 두 손가락: 이동, 휠 / 핀치: 확대, 더블클릭: 초기화 (관성 있음).
- 피동 풀리를 중심선 방향으로 끌기: 축간 거리 변경 (유효한 벨트 길이로 스냅, 입력값 즉시 갱신).
- 출력 풀리 테두리를 끌기: 출력축을 돌려 부하 시 변형 표시 (놓으면 설계 부하로 복귀).
- 툴바: 재생 / 정지, 속도 (기본 0.25×, 1× = 모터 60 rpm), 시간 슬라이더 (출력 1회전), 장력 / 변형 / 하중 / 치수 토글, 카메라 프리셋 (원근 / 정면 / 측면 / 풀리 확대), 언어, 설정 패널.

### 로컬 빌드
```sh
rustup target add wasm32-unknown-unknown
cargo test                 # 코어 + 렌더러 수학 테스트
trunk build --release      # dist/ 생성 (public_url = /belt-drive/)
sh tools/privacy_check.sh  # "clean" 출력 확인
```
`dist/`를 `/belt-drive/` 경로로 정적 서버에 올리면 됩니다.

### 라이선스
MIT — `LICENSE` 참고 (copyright belt-drive contributors).

---

## English

### What it is
A single-page app for a 1- or 2-stage timing-belt reduction in a robot joint (a quasi-direct-drive
leg, a wrist, …): which pulley teeth, which belt length and centre distance, how wide the belt must
be, how much pretension, how stiff it is, and how much backlash-free stiffness you give up compared
with a gearbox. Everything runs in your browser — no server, no analytics, no external requests.

- Four belt profile presets (GT-style 2 mm / 3 mm, HTD-style 5 mm, T-style 5 mm steel cord), all
  labelled "typical": generic values, not any maker's catalogue.
- Inputs per stage (driver / driven teeth, target centre distance) plus belt width, motor torque and
  max speed, joint output peak torque, pretension factor and service factor.
- Outputs: real centre distance for a whole-tooth belt, wrap angle, teeth in mesh, tensions, width
  needed, belt speed, span frequency, output stiffness and deflection, shaft load, with a status per
  check (OK / warning / fail) and overall.
- Find reductions: for a target total ratio and tolerance, the top 12 combinations that pass every
  check (click a row to load it).
- Plots: output stiffness and deflection vs belt width; T1 / T2 vs output torque with the T2 = 0
  tooth-jump limit.
- WebGL2 3D view: toothed pulleys, flanges, shafts, motor, frame, animated tooth-block belt, tension
  colouring, exaggerated sag / deflection, shaft-load arrows, dimension labels. Without WebGL2 a
  message is shown and the numbers and plots keep working.
- Korean / English (browser default), mobile friendly, follows the system light / dark scheme.

### Model and assumptions
Textbook relations only:
- Pitch diameter d = z·p/π; stage ratio z2/z1; total ratio is the product.
- Open-belt length L = 2√(C² − e²) + π(D + d)/2 + (D − d)·asin(e/C), e = (D − d)/2. The belt with the
  whole tooth count nearest to the target C is chosen, and the real C is solved by Newton's method.
- Wrap on the small pulley θ = π − 2·asin((D − d)/2C); teeth in mesh = ⌊z·θ/2π⌋ (warn < 6, fail < 3).
- Design load: output torque = max(required peak, motor torque × ratio), reflected to each stage's driver.
- Effective tension Te = 2T/d; centrifugal Tc = m'·v²; pretension Ti = factor × (allowable tension per
  mm × width); T1 = Ti + Te/2, T2 = Ti − Te/2 (T2 ≤ 0 → tooth-jump risk → fail). With the default
  factor 0.5, T2 reaches zero when Te exceeds the belt's allowable working tension.
- Width needed b = Te·service factor / (allowable per mm − m' per mm·v²).
- Belt speed v = π·d1·n/60 and the motor rpm limit from the profile's max belt speed.
- Span frequency f = (1/2L)·√(Ti/m'), for setting pretension with a phone tuner app.
- Span stiffness k = EA/L_span; both spans act while the slack side stays taut: K = 2k·r². Two stages
  are springs in series, stage 1 reflected to the output by the square of the stage-2 ratio.
  Deflection = load / K (arcmin).
- Shaft load = vector sum of T1 and T2: √(T1² + T2² − 2·T1·T2·cos θ).

Assumptions: inextensible geometry, linear elastic spans, no tooth deformation / backlash model,
generic typical values. Belt life, idlers, serpentine drives and thermal effects are out of scope.

### Controls
- Left drag: orbit; right drag / two fingers: pan; wheel / pinch: zoom; double-click: reset (with inertia).
- Drag a driven pulley along the centre line: changes the centre distance (snaps to a valid belt
  length, inputs update live).
- Drag the output pulley rim: rotates the output to show loaded deflection (springs back to the
  design load on release).
- Toolbar: play / pause, speed (default 0.25×, 1× = motor at 60 rpm), time slider (one output
  revolution), tension / deflection / force / dimension toggles, camera presets (perspective /
  front / side / pulley close-up), language, settings panel.

### Local build
```sh
rustup target add wasm32-unknown-unknown
cargo test                 # core + renderer math tests
trunk build --release      # writes dist/ (public_url = /belt-drive/)
sh tools/privacy_check.sh  # should print "clean"
```
Serve `dist/` under `/belt-drive/` from any static file server.

### License
MIT — see `LICENSE` (copyright belt-drive contributors).
