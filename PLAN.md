# 구현 계획

이 문서는 Pkl **0.32.0** Rust 재구현의 단계별 로드맵, 실행 순서, 산출물, 완료 증거를 함께 관리합니다. 목표와 범위는 [INTENT.md](INTENT.md), 설계 원칙은 [아키텍처](docs/architecture.md), 판정 기준은 [TESTING.md](TESTING.md)를 따릅니다.

## 현재 상태

Git 저장소, Rust workspace, 참조 submodule, 개발 문서와 초기 구성 검증 도구가 준비되어 있습니다. M1에서 제한된 parser·resolver·lazy VM·heap·Host와 실제 Node·브라우저 호출 경로를 구현했습니다. 전체 Pkl 언어·표준 라이브러리·CLI·REPL·server·제품 API는 아직 구현되지 않았습니다.

M0의 참조 소스·Java oracle 고정, 기능·코퍼스 목록, 원시 차등 실행 도구, Pantry 조사, 초기 Java 성능 측정과 CI 구성을 완료했습니다. 최종 코드 커밋 `2164ffa`의 원격 CI 작업 4개가 모두 성공했고, 내려받은 원시 증거와 Java 재비교 결과도 검증했습니다. [CI 확인 기록](reports/m0/ci-summary.json)을 참조하세요. 초기 Rust 빌드와 테스트, Java 재실행 결과는 Rust 언어 호환성이나 성능 목표 달성의 증거가 아닙니다.

전체 명령 코퍼스 1,112개 중 실행 가능한 1,013개를 모두 호출했습니다. 1,012개 결과 기록·99개 실행 조건 차단·1개 Java 힙 실패로 엄격한 전체 판정은 실패입니다. 2 GiB 힙의 별도 실험은 성공했으며 기존 512 MiB 실패를 대체하지 않습니다. 자세한 결과는 [M0 증거 보고서](reports/m0/README.md)에 모읍니다.

## 단계별 로드맵

아래 표는 단계별 상태와 완료 기준의 요약입니다. 작업별 산출물과 구체적인 종료 조건은 뒤의 M0–M7 절에서 관리합니다.

| 단계 | 현재 상태 | 선행 조건 | 주요 산출물·완료 기준 |
| --- | --- | --- | --- |
| 초기 구성 | 구축됨 | 없음 | Git·Rust workspace·참조 submodule·개발 문서와 초기 구성 검증 |
| M0 기준선 | 완료: 로컬 기준선·원격 CI 검증 확인 | 초기 구성 | oracle·목록·원시 비교 도구·Java 초기 측정과 전체 처리 상태 확보; 판정 보류와 미구현 범위 보존 |
| M1 구조 검증 | 완료: 구조·Graal 조사·원격 CI 증거 확인 | M0의 oracle·실행 계약·초기 기준선 | 제한된 공통 엔진·실제 Node/브라우저·원시 비교와 비용 측정 완료. 후보 기각과 알려진 호환성·성능 실패는 후속 필수 조건으로 보존 |
| M2 언어 런타임 | 진행 중: 공통 VM·문법 확대와 차등 검증 | M1의 이름 해석·값·heap·진단 설계 결정 | 객체·lazy 평가·상속·closure·타입·Reference 구현; 언어 코퍼스의 결과·오류 바이트 일치 |
| M3 표준 라이브러리·출력 | 계획됨 | M2의 해당 언어 기능 | 전체 builtin·renderer·reflection·Pkl Binary 구현; 공개 멤버 누락 0, 출력·오류 일치 |
| M4 외부 기능 | 계획됨 | M1 Host 계약과 M2·M3의 해당 기능 | module/resource Host·HTTP·프로젝트·패키지·external reader 구현; 실제 I/O·패키지 코퍼스 통과 |
| M5 제품 API | 계획됨 | M2·M3·M4의 해당 기능 | CLI·REPL·server·Rust API·formatter·문서 생성·코드 생성·JVM adapter 구현; 기존 호출·client·수명주기 호환 |
| M6 Wasm·N-API 완성 | 계획됨 | M1의 실행환경 실험과 M2–M5의 해당 공통 기능 | 비동기 Host·adapter·배포 패키지 완성; 실제 브라우저·Node의 대상 코퍼스 통과 |
| M7 릴리스 검증 | 계획됨 | M0–M6의 필수 산출물 | 전체 Pantry 코퍼스·플랫폼 검증과 실행 시간·메모리 보고서; 필수 범위의 알려진 불일치 0, 성능·메모리 기준 충족 |

Wasm 빌드와 실제 브라우저·Node의 작은 실행 테스트는 M1부터 구축하고, M6에서 전체 기능으로 확대합니다. 성능과 메모리 측정도 M1부터 구현과 병행합니다.

독립적인 테스트 목록 조사와 adapter 개발은 병행할 수 있습니다. 단계의 시작과 완료는 구분하며, 후속 단계 착수가 선행 단계 완료를 뜻하지 않습니다. 일정과 작업 배분은 M0의 규모 조사와 M1의 실험 결과를 확보한 뒤 정합니다.

## 실행·판정 원칙

- 소스 버전의 기준은 [references.lock](references.lock)과 `external/`의 gitlink입니다. 실행 artifact의 버전·해시·빌드 조건도 별도로 고정합니다. 참조 저장소 변경은 버전 갱신 작업으로만 수행합니다.
- `../pkl-rs.rev1`은 설계와 회귀 입력의 참고 자료입니다. 가져올 구현은 0.32.0 의미론과 비교하고, 가져온 입력의 기대 결과는 고정된 Java oracle로 검증합니다.
- Native·Wasm·N-API는 평가기, 값 모델, 진단, renderer를 공유합니다. 외부 I/O는 Host로 분리하며 제품 평가 경로에 Java fallback을 추가하지 않습니다.
- 작업 상태는 `계획됨`, `진행 중`, `차단됨`, `완료`로 기록합니다. 차단 사유와 해제 조건을 남기며, 완료에는 산출물과 검증 증거가 필요합니다.
- 검증 결과는 `통과`, `실패`, `미실행`, `미구현`, `환경 오류`, `판정 보류`로 구분합니다. 임시 예외에는 범위·이유·해제 조건을 남기고 필수 범위의 완료 분모에서 제외하지 않습니다.
- 새 불일치는 입력을 최소화하고 oracle 출처·재현 환경·실제 바이트를 보관한 뒤 필수 회귀 사례에 추가합니다. Rust 결과로 기대 결과를 덮어쓰지 않습니다.
- 기본 문서와 설계 기록은 영어, 이 문서를 포함한 계획서는 한국어로 유지합니다. upstream 자료와 fixture 원문은 보존합니다.

## M0 — 호환성·성능 기준선

**목적:** 구현 성과를 판단할 수 있는 재현 가능한 Java 기준과 누락을 검출하는 실행 도구를 마련합니다.

### 진행 기록

| 작업 | 상태 | 증거·남은 작업 |
| --- | --- | --- |
| M0-01 | 완료 | [Oracle 계약](docs/oracle.md), `oracle.lock.json`, [실행 증거](reports/m0/oracle-evidence.tar.gz). 공식 JAR 재다운로드·checksum 변조 거부, 성공·오류 원시 바이트 재현, 회귀 검사 13개 통과. 성능용 GraalVM은 macOS arm64 고정이며 다른 플랫폼 확대는 별도 작업입니다. |
| M0-02 | 완료 | [소스 목록](docs/inventory.md): 985개 파일·11,624개 항목, external 선언·builtin 등록 후보 각 542개 양방향 대응. 재생성·누락 검출과 회귀 검사 18개 통과. 소스 목록이며 컴파일된 ABI 증명은 아닙니다. 구현·행동 테스트 연결은 미구현·미연결 상태로 보존합니다. |
| M0-03 | 완료 | [코퍼스 계약](docs/corpus.md): Pkl 파일 1,546개·project 파일 73개·명령 사례 1,112개와 전체 리소스 hash. 실행 준비 1,013개·Host/adapter 차단 99개, JVM 테스트/지원 소스 199개와 shell suite 1개를 별도 기록. [rev1 회귀 3개](docs/regressions.md)를 원문·라이선스와 함께 별도 선별하고 Java 0.32.0 기록·정확한 재비교 완료. discovery 회귀 검사 8개·선별 입력 검사 4개 통과. |
| M0-04 | 완료 | [실행 도구 계약](docs/compatibility-tooling.md), 원시 stdout·stderr·종료 코드·생성 파일 비교, 입력 hash·실행 누락·timeout 검출. 회귀 검사 19개와 실제 Java smoke 7개 기록·Java 재비교 통과. [기록 증거](reports/m0/tooling-smoke-evidence.tar.gz), [재비교 증거](reports/m0/tooling-replay-evidence.tar.gz). 현재 프로세스 실행 adapter는 POSIX이며 Windows는 명시적 미지원입니다. |
| M0-05 | 완료 | [Pantry 결과](reports/m0/pantry-summary.json), [원시 증거·패키지·cache](reports/m0/pantry-evidence.tar.gz). 29개 project의 패키지/API 테스트 성공, 생성 ZIP 모듈 206개 실행(성공 142·설정 등이 필요한 오류 관찰 64). 다중 출력 4회·설정된 generator 2회·tar pipeline 추가 실행. 원격 의존성 7개 checksum 검증, 실제 cache 72개 파일 보관. 알려진 live GitHub 입력과 잠정 오류는 미검증 범위로 유지합니다. 회귀 검사 5개 통과. |
| M0-06 | 완료 | [측정 계약](docs/benchmarks.md), [측정 결과](reports/m0/benchmarks.json), [원시 증거](reports/m0/benchmark-evidence.tar.gz). 6개 workload·cold CLI 표본 30개·독립 JVM 평가기 실행 24회, 원시 결과 검증과 warmup 수렴 확인. 재귀 함수는 Graal 컴파일 중단이 관찰되어 완전 최적화 여부 판정 보류. 첫 timeout 설정 실험의 힙 실패도 별도 보존. 도구 회귀 검사 9개 통과. |
| M0-07 | 완료 | [CI 계약](docs/ci.md), [전체 결과](reports/m0/README.md), [로컬 검증 증거](reports/m0/validation-evidence.tar.gz), [원격 CI 증거](reports/m0/ci-evidence.tar.gz). 로컬 검증 11개 명령·Python 회귀 검사 79개 통과. `2164ffa`의 [CI 실행](https://github.com/luuvish/pkl-rs/actions/runs/39364517355)에서 Linux·macOS·Windows workspace와 Java tooling 작업 모두 성공. artifact checksum·원시 capture·smoke 재비교 7개를 내려받아 다시 검증했습니다. 전체 코퍼스의 알려진 실패 상태는 별도로 보존합니다. |

### 후속 단계로 넘기는 확인 항목

- 전체 코퍼스의 99개 차단 항목은 formatter·package/project·TLS/HTTP·classpath adapter와 fixture를 구현할 때 해제합니다. JVM 테스트/지원 소스 199개의 실제 API 실행도 별도로 연결합니다.
- 대형 List 사례는 512 MiB 실패와 2 GiB 성공 증거를 함께 검토한 뒤 공식 사례별 자원 계약을 결정합니다. 현재 전체 gate의 실패 상태는 유지합니다.
- M1 [Graal 마감 조사](docs/m1-graal.md)에서 재귀 함수의 컴파일 중단 원인과 실제 Tier 2로 컴파일된 workload를 확인했습니다. 이번 기준선으로 완전 최적화 성능이나 Rust 우위를 주장하지 않습니다. 장기 메모리·제품 API·Wasm·N-API 등 미구현 측정 항목도 유지합니다.

### 작업 계약

| 작업 | 산출물 | 완료 증거 |
| --- | --- | --- |
| M0-01 Oracle 고정 | Java 0.32.0 실행 artifact, checksum, 출처·빌드 절차, JDK·Graal/Truffle 설정과 라이선스 기록 | 새 환경에서 같은 artifact와 버전을 확인하고 성공·오류 사례를 재실행 |
| M0-02 기능·API 목록 | 언어·stdlib 공개 멤버·builtin·renderer·CLI 명령과 옵션·protocol·공개 API별 구현 및 테스트 연결표 | 고정된 upstream 선언과 목록을 대조하고 누락 검출 방식 확인 |
| M0-03 테스트 discovery | upstream, Pantry, rev1 회귀 입력의 출처와 실행 계약을 담은 manifest | 파일 수·실행 사례 수·assertion 수를 구분하고 모든 발견 항목의 처리 상태 기록 |
| M0-04 차등 실행 도구 | 실행기 adapter, 원시 출력 수집, 바이트 비교, 실패 재현 자료와 결과 보고서 | stdout·stderr·종료 코드·생성 파일의 누락과 차이를 의도적으로 주입하여 실패 확인 |
| M0-05 Pantry 정합성 | 고정 revision의 패키지·예제·테스트 실행 목록, 의존성·외부 입력 고정 | Java 0.32.0 실행 결과 확보, 실행 불가 사례의 원인·처리 상태 명시 |
| M0-06 성능 계약 | 필수 workload 목록, 측정 범위·장비·통계·메모리 수집 방법, Java 초기 기준선 | cold CLI와 충분히 warmup한 상주 evaluator를 반복 측정하고 원시 자료 보관 |
| M0-07 실행 통합 | CI의 목록·실행 결과 대조, 검증 도구 자체 회귀, 재현 절차 | 빈 코퍼스·기대 결과 누락·oracle 부재·skip·부분 실행이 성공 보고로 이어지지 않음 |

테스트 manifest에는 `case_id`, 저장소·revision·원본 경로, 입력·리소스 hash, 인자·환경, 필요한 Host 기능, 예상 성공 또는 오류, 비교 채널·파일, 실행 target을 기록합니다. 입력 파일이 여러 사례를 만들거나 여러 파일이 한 사례를 구성하는 관계도 보존합니다.

차등 실행은 작업 경로, locale, 시간대, 터미널, 환경변수와 네트워크 응답을 고정합니다. stdout·stderr와 생성 파일을 원시 바이트로 비교하며 오류의 공백·밑줄·소스 위치·호출 프레임·hint·발생 순서도 포함합니다. `trim`, 정렬, 재직렬화, 경로 문자열 치환으로 차이를 없애지 않습니다.

서버의 무작위 evaluator ID는 고정 ID를 주입한 codec 테스트에서 전체 바이트를 비교합니다. 실제 세션에서는 발급 ID의 참조 일관성과 결과 payload를 검사하며 전체 세션의 바이트 일치로 보고하지 않습니다.

Pantry는 고정 commit만으로 호환성이 확인되지 않습니다. 개발 트리와 패키징된 모듈의 실행 조건, 의존성, 예제 출력과 생성 파일을 조사하고 Java 실행 결과를 확보합니다. 실행 불가 사례를 조용히 제외하지 않으며 참조 submodule을 수정해 통과시키지 않습니다.

아직 Rust evaluator가 없으므로 M0에서는 Java 결과와 실행 도구의 오류 검출을 검증합니다. 실행기 계약 테스트에 사용하는 합성 출력은 Pkl 호환성 결과로 집계하지 않습니다. 실제 Rust 차등 비교는 해당 기능 구현과 함께 연결합니다.

**종료 조건:** oracle 재현 절차, 기능·테스트 목록, 원시 비교 도구, Java 기준 자료와 초기 성능 측정이 검토 가능해야 합니다. 미실행과 환경 오류를 포함한 전체 상태가 보고되어야 하며, M0 완료는 Pkl 구현 완료를 뜻하지 않습니다.

## M1 — 핵심 구조와 호환 위험 검증

**목적:** 런타임 전체에 영향을 주는 선택을 작은 구현과 Java 비교로 검증합니다. 아래 실험은 Native와 초기 Wasm 실행을 함께 고려합니다.

### 실행 항목과 마감 기준

M1은 아래 항목별 구현·비교 증거·설계 결정이 갖춰진 뒤 완료로 표시합니다. 초기 prototype의 지원 문법과 probe 범위를 명시하고, 미지원 기능을 성공 처리하지 않습니다. 필수 실험의 실행·증거 수집이 실패하거나 플랫폼별 결과가 다르면 해당 항목을 미완료로 유지합니다. 비교에서 발견한 불일치는 후보의 부적합 판단 근거로 보존하며 호환성 통과로 바꾸지 않습니다. 후보를 기각한 경우 공통 대안의 계약과 근거를 함께 기록해야 구조 검증을 마칠 수 있습니다.

| 항목 | 현재 상태 | 마감에 필요한 증거 |
| --- | --- | --- |
| M1-01 SourceMap·parser | 완료 | [프런트엔드 증거](reports/m1/frontend.json): 제한된 Java/Rust 비교 15/15, UTF-8·UTF-16·code point와 EOF underline 구분. 전체 문법은 M2입니다. |
| M1-02 이름 해석·진단 | 완료 | lexical binding·receiver·owner·this·super의 정적 계약과 Java 값·첫 parser 오류 비교. [계약](docs/m1-frontend.md)에 전체 진단 renderer 미구현을 명시했습니다. |
| M1-03 숫자·정규식 | 완료 | [6,078개 비교](reports/m1/semantics.json)와 Native·Node·Wasm 공통 결과 확보. 숫자 표본은 모두 일치, regex 후보는 Unicode·UTF-16 차이 2개로 기각하고 별도 cursor 증거를 갖춘 공통 대안을 선택했습니다. 숫자 비용 96개 결과 사전 일치·6개 프로세스 수렴 확인: 현재 formatter 약 6.1배 느림을 후속 성능 gate로 남깁니다. |
| M1-04 lazy 값·heap | 완료 | 실패를 영구 캐시하지 않도록 수정했습니다. [마감 차등 증거](reports/m1/lifecycle.json)의 retry protocol 전체 바이트가 Java와 일치하며 첫 오류 순서·복구·성공 의존성 캐싱을 확인했습니다. runtime 회귀 13개 통과, 진단 바이트 차이는 별도 실패로 보존합니다. |
| M1-05 VM·캐시 | 완료 | [일반 VM](reports/m1/runtime-performance.json)은 Java API 대비 약 1.27배 느림. [정수 특수화](reports/m1/runtime-specialized.json)는 약 0.46배, 일반 Rust 대비 약 2.75배 개선. M2의 제한된 기반으로 특수화 bytecode를 선택합니다. [Graal 마감 조사](docs/m1-graal.md)에서 원래 산술·재귀 함수의 프레임 병합 중단 원인과 원본 JAR의 실제 Tier 2 workload를 확인했습니다. 기존 비율은 컴파일 실패 당시 측정이며 전체 성능 gate는 별도입니다. |
| M1-06 Host·수명주기 | 완료 | 같은 VM·heap·성공 member cache를 공유하는 중첩 context를 구현했습니다. [재진입 protocol](reports/m1/lifecycle.json) 바이트가 Java와 일치하며 Host 회귀 15개가 root·token·deadline·종료를 검증합니다. 실제 JS에서 진행 중인 단일·중첩 Host 작업의 abort·Promise 종료·timer/listener/map 해제를 확인했습니다. |
| M1-07 브라우저·Node | 완료 | [마감 플랫폼 결과](reports/m1/platform-closing.json): Native·Node·Chromium의 6,078개 primitive 바이트 모두 동일, Node·브라우저 각각 필수 시나리오 14개 통과. Java 비교는 각 6,076개 일치·동일한 정규식 실패 2개입니다. |
| M1-08 통합·기록 | 완료 | Graal 조사 `bb80685`와 CI 오류 전달 보완 `4cd9c08`을 푸시했고 [최종 코드 CI 5개 작업](https://github.com/luuvish/pkl-rs/actions/runs/39378401720)이 모두 성공했습니다. 원격 artifact의 SHA-256·커밋 소스·원시 결과와 9개 Graal 증거 재검증을 확인했습니다. [최종 CI 기록](reports/m1/ci-graal-summary.json)·[증거](reports/m1/ci-graal-evidence.tar.gz), [로컬 검사 13개·Python 141개](reports/m1/validation-graal.json)를 보관했습니다. 앞선 `6a5e4b5`의 [CI 증거](reports/m1/ci-closing-summary.json)도 유지합니다. |

구문·이름 해석과 VM·heap은 각각 `gpt-6.1-sol / high`, 숫자·정규식 실험은 `gpt-6-astra / high` 에이전트가 담당합니다. 주 에이전트는 Host·플랫폼 연결·통합 검증을 담당하며 공유 인터페이스와 Cargo 설정을 관리합니다. 기능 검증은 병행하고 성능 측정은 호스트를 비운 상태에서 순차 실행합니다.

마감 보완은 공유 VM·Host 구현에 `gpt-6-astra / high`, Java 차등 검증과 Node·브라우저 검증에 각각 `gpt-6.1-sol / high`를 배정합니다. 주 에이전트는 코드 검토·증거 보관·문서·CI 통합을 담당합니다. 구현과 검증 통합, 최종 CI 증거 보관을 단계별로 커밋·푸시합니다. M1-04와 M1-06의 미달 항목이 닫히기 전에는 M1 전체를 완료로 보고하지 않습니다.

앞선 lazy·Host 마감 보완과 독립 코드 검토, 로컬·원격 증거 검증을 완료했습니다. 재검토에서 원래 M1에 배정한 Graal 중단 원인·실제 컴파일 workload 조사가 남아 있음을 확인하여 M1-05를 다시 열어 조사했습니다. 원인 분석은 `gpt-6-astra / high`, workload 실험과 증거 도구는 각각 `gpt-6.1-sol / high`에 배정했습니다. 원본 실패·재컴파일 대조군·진단 우회·원본 JAR 성공을 구분한 9개 사례와 컴파일러 그래프를 확보했고, [로컬 검사 13개·Python 141개](reports/m1/validation-graal.json)가 통과했습니다. 추가 원격 CI 5개 작업과 내려받은 증거 검증까지 완료하여 M1을 제한된 핵심 구조 실험 범위에서 종료합니다. 전체 언어/API 호환성과 성능 목표는 아래 후속 조건 및 M2 이후의 완료 기준으로 남습니다.

| 실험 | 확인할 내용 | 설계 결정에 필요한 증거 |
| --- | --- | --- |
| Parser·SourceMap·resolver | UTF-8 위치, Java의 UTF-16 위치, Pkl code point 연산; 0.32의 정적 이름 해석; lexical binding·receiver·owner·`this`·`super` | Unicode·잘못된 입력·이름 충돌 사례의 값, 위치, 첫 오류 일치 |
| 정규식 | Java Pattern 문법, Unicode, capture·치환, 오류 문구 | 대표 경계 코퍼스의 Native·Wasm 결과와 오류 바이트 비교 |
| 숫자·수학 | overflow, `Double.toString`, `StrictMath`, NaN, 음수 0, subnormal, 반올림 | 경계값과 생성 입력의 값·출력 비교 및 비용 측정 |
| Lazy 값·heap | 미평가·평가 중·완료·실패 상태, 상속·amendment, memoization, 순환 객체 회수 | forcing·오류 순서 일치와 반복 세션의 메모리 회수 자료 |
| VM·캐시 | bytecode VM 후보, 객체 배치, dispatch, specialization, 컴파일 결과와 평가 상태의 수명 분리 | 올바른 결과를 낸 prototype과 warm JVM의 실행 시간·메모리 비교 |
| 비동기 Host·수명주기 | 중단·재개, 재진입, 취소·timeout·close, 캐시와 외부 입력 | 같은 평가 상태에서의 재개와 종료 후 자원 회수 사례 |

설계 기록에는 검증한 입력, upstream 근거, 원시 결과, 후보별 장단점, 미해결 항목과 다음 검증을 남깁니다. regex나 숫자 구현이 특정 플랫폼에서만 일치하면 공통 해결책의 근거가 확보될 때까지 해당 결정을 미완료로 둡니다.

이 단계부터 실제 브라우저와 Node의 작은 호출 경로를 구성합니다. 공통 engine 연결, 문자열·정수·바이트 전달, 비동기 Host 재개와 오류 전달을 확인합니다. Wasm 컴파일 성공만으로 이 실험을 대체하지 않습니다.

**종료 조건:** 이름 해석·값·heap·진단·Host의 계약을 확정하고 주요 위험에 대한 비교 증거를 확보합니다. bytecode VM을 채택할지 또는 실행 전략을 재검토할지 측정으로 결정합니다. 미달한 실험을 숨기거나 성능 기준을 완화하여 통과시키지 않습니다. 제한된 prototype의 성공은 전체 성능 목표 달성을 뜻하지 않습니다.

### M1에서 확인한 후속 구현·검증 조건

- regex의 현재 backend는 Java Unicode 버전과 supplementary 문자 내부의 zero-width 위치에서 실패합니다. Java 21의 Unicode table과 UTF-16 cursor를 갖춘 공통 matcher를 구현하고, 기존 실패 2개를 포함한 전체 비교를 통과해야 합니다. 별도의 빈 pattern·dot cursor 실험은 이 구현 전체의 완료 증거가 아닙니다.
- M1 마감 시 실행 오류 3개의 전체 stderr와 lifecycle 실험의 오류 payload 4개가 Java와 달랐습니다. M2의 공통 진단 경로를 연결한 재검사에서는 실행 오류 2개가 일치했고 순환 참조와 lifecycle payload 차이는 남았습니다. M2의 진단 renderer·논리적 frame·오류 선택과 함께 나머지 바이트 차이를 없애야 합니다. missing-super 동작도 언어 의미론 검증에 포함합니다. 실패 재시도는 마감 실험에서 수정·검증했습니다.
- 중첩 context·실패 복구·종료 시 취소 전달은 작은 구현으로 검증했습니다. 제품 module/resource cache와 공개 API는 후속 구현입니다. Java 0.32.0은 callback 안에서 새 resource를 읽으면 `ConcurrentModificationException`에 의한 `PklBugException`을 낼 수 있습니다. [Java-only 실패 관찰](reports/m1/lifecycle.json)을 보존하며 M4 cache 구현에서 이 동작을 다룹니다. JS의 unpaired surrogate와 장기 clock 범위도 제품 지원 전에 해결해야 합니다.
- 숫자 formatter는 선택한 primitive 비용 측정에서 Java보다 약 6.1배 느립니다. 정확한 출력 바이트를 유지하며 알고리즘·할당 비용을 개선하고 같은 입력·조건에서 다시 측정해야 합니다.
- 특수화 VM의 작은 정수 workload 개선은 전체 성능 통과가 아닙니다. 고정 Graal/Pkl 조합의 `FrameWithoutBoxing` 중단 원인과 실제 Tier 2 workload는 [마감 조사](docs/m1-graal.md)에서 확인했습니다. 원인 확인용 우회는 의미론을 생략하므로 기준 oracle에 적용하지 않습니다. 실제 컴파일된 Java와 같은 입력·호출 경계로 새 성능 측정을 수행하고, 큰 lazy 객체·장기 세션·제품 API를 비교해야 합니다.

## M2 — 언어 런타임

M1의 공통 VM을 확장합니다. VM·heap·호출 의미론은 `gpt-6-astra / high`, parser·resolver와 검증 도구는 각각 `gpt-6.1-sol / high`에 배정합니다. 주 에이전트가 공통 진단 렌더러, 통합 검증, 문서와 커밋·원격 CI를 담당합니다.

### 실행 항목과 진행 상태

| 작업 | 상태 | 구현·검증 범위와 남은 조건 |
| --- | --- | --- |
| M2-01 전체 목록과 실행 판정 | 진행 중 | `compatibility/m2-language.json`에 의미론 분류·upstream 선언·언어 snippet·rev1 후보를 보존합니다. 선언 수는 기능 커버리지가 아니며, 미연결 항목은 전체 gate 실패로 남깁니다. |
| M2-02 lexer·표현식·문자열 | 진행 중 | 숫자·연산자 우선순위·조건식·nullable 연산·raw/multiline/interpolation과 0.32 continuation을 구현했습니다. 고정 Java의 UTF-16 식별자 표, CR/CRLF, 지연 AST 검증 순서를 연결했습니다. 전체 유효·오류 문법 코퍼스 연결이 필요합니다. |
| M2-03 함수·closure·호출 | 진행 중 | 별도 method namespace, 재귀, lexical capture, 매개변수·반환 타입 검사, `\|>` 호출과 함수형 제약을 구현했습니다. Host 중단·재개 중 GC를 검증하며 전체 함수 타입·amendment·오류 조건은 후속 항목입니다. |
| M2-04 객체·클래스·상속 | 진행 중 | 클래스·prototype·상속·super 메서드·const capture·runtime identity와 타입 초기화 순서를 연결했습니다. 전체 modifier·override·초기화 재시도·상속 순환 및 eager/lazy 규칙의 완료가 필요합니다. |
| M2-05 타입·제약·generic | 진행 중 | primitive·nullable·union·클래스 타입과 generic alias·기본 값을 구현했습니다. 같은 VM의 제약 continuation, custom `this`, 상속된 제약의 선언 owner, alias 실행 환경, 복구 가능한 불일치와 실행 오류 구분을 연결했습니다. generic collection의 lazy wrapper·eager key/List 검사·캐시 재사용을 연결했습니다. 함수/구조 타입·recursive 타입·전체 제약 코퍼스는 남아 있습니다. |
| M2-06 collection·generator | 진행 중 | 순서 있는 객체 본문, `Dynamic`·`Listing`·`Mapping`의 element·entry·subscript·amendment, `for`·`when`·spread와 `List` 생성 경로를 연결했습니다. 지연 멤버·반복 변수 capture·source 선행 강제 평가·상속 순서·Host 중단 중 GC를 검증합니다. generic wrapper와 일부 inferred new·클래스 기본 값을 연결했습니다. member predicate의 부모 순회·선택·amendment·지연 본문과 선언 owner 캐시를 추가했습니다. 구조 동등성과 객체 키를 확장하며 전체 해시 충돌·순환 키·typed generator·custom default 규칙은 남아 있습니다. |
| M2-07 Reference·메타데이터 | 계획됨 | `Reference<Domain, Type>` 및 reflection에 필요한 module/class/member/type 정보를 구현합니다. |
| M2-08 진단·첫 오류·논리적 스택 | 진행 중 | 공통 uncolored renderer에 제약·union 진단, 선택적인 power assertion 재실행·관찰값 그래프, 값을 강제하지 않는 객체 표시와 UTF-16 길이 제한을 연결했습니다. 재귀 frame 압축·전체 method 진단·전체 오류 선택은 남아 있습니다. |
| M2-09 생성 검사·플랫폼·완료 gate | 진행 중 | Java 차등 입력·metamorphic 관계·seeded 생성과 실제 native/Node/browser 실행을 연결합니다. 작은 선택 코퍼스 성공은 M2 전체 완료가 아닙니다. |

`|>`와 타입 제약에 이어 객체 본문을 순서 있는 멤버 모델로 확장하고 element·entry·generator·spread를 연결했습니다. generic collection wrapper와 member predicate를 추가했으며 다음 구현에는 나머지 collection 규칙, 함수/구조 타입, `Reference`·메타데이터, 전체 진단·upstream 코퍼스가 필요합니다. 현재 선택 사례의 성공은 원래 M2 범위나 종료 조건을 줄이지 않습니다.

타입 제약의 실행 상태·alias 실행 환경·GC와 선택적인 관찰 명령은 [아키텍처](docs/architecture.md)에 기록했습니다. 동일 alias를 타입 인자로 중첩한 오류 span은 새 회귀 사례로 확인했습니다. 클래스 초기화 실패 후 재시도 및 부분 초기화의 전체 규칙, 탐색 과정에서 드러난 Java alias JVM 버그와 미구현 Dynamic method 진단은 별도의 미해결 범위로 유지합니다.

### 원래 범위와 종료 조건

첫 구현과 클래스·타입 별칭 단계는 `fb5a348`, `281ef6c`에 커밋·푸시했고 각 원격 CI 6개 작업과 내려받은 증거를 검증했습니다. 원래 [두 번째 단계 기록](reports/m2/stage2-summary.json)도 유지합니다.

타입 제약을 확장한 [세 번째 단계 최종 로컬 검증](reports/m2/stage3-final-summary.json)에서는 선택 사례 294개 중 292개가 Java와 원시 바이트로 일치했습니다. 순환 참조 오류와 정수 `% 0`의 Java 버그 출력은 실패로 유지하며 새 예외는 추가하지 않았습니다. Metamorphic 관계 17개, Rust 검사 140개, Python 검사 170개, Node·브라우저 각 20개 시나리오가 통과했습니다. 제약 안의 Host 중단·재진입·GC·취소·재시도·종료·시간 초과와 power assertion 재실행 순서도 검증했습니다. 마지막 독립 검토에서 상속된 제약의 잘못된 반환값 오류가 선언·재정의 프레임을 잃는 문제를 수정하고 회귀 사례 2개를 추가했습니다. 수정 전 실패와 이전 292개 선택 기록도 별도로 보존합니다. 전체 목록의 클래스·타입 나머지 규칙, generator·Reference 및 미연결 코퍼스가 남아 있으므로 **M2는 진행 중**입니다. 개발 gate 성공과 엄격한 전체 gate 실패를 구분합니다.

세 번째 단계 구현은 `b00e508`, 최종 오류 프레임 수정은 `5d58e76`에 커밋·푸시했습니다. 최종 구현의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39396439537) 작업 6개가 모두 성공했습니다. [내려받은 증거 검증](reports/m2/ci-stage3-summary.json)에서 두 artifact의 원본 ZIP 해시와 소스 스냅샷 541개(고유 파일 394개)를 정확한 커밋과 대조했습니다. CI의 선택 결과도 292개 일치·나머지 2개 실패이며, 전체 gate는 실패로 유지합니다.

네 번째 단계는 런타임·GC·평가 순서를 `gpt-6-astra / high`, 프런트엔드·독립 검토와 차등 검증을 각각 `gpt-6.1-sol / high`에 배정했습니다. `4512488`에 순서 있는 collection·generator 구현과 회귀 검사를 커밋·푸시했습니다. [최종 로컬 검증](reports/m2/stage4-final-summary.json)은 선택 사례 415개 중 413개가 Java와 원시 바이트로 일치했고, 기존 순환 참조의 oracle 실패와 `% 0` 차이를 유지했습니다. 새 사례 121개와 metamorphic 관계 20개, Rust 163개·Python 171개 검사, 실제 Node·브라우저 각 23개 시나리오가 통과했습니다. source 선행 강제 평가·상속 순서·spread 값 보존·local 중복·module/class 분류·생성 중 Host 중단과 GC의 회귀도 포함했습니다. 원래 294개 선택 입력은 변경하지 않았으며, 수정 전 실패는 [탐색 증거](reports/m2/exploration-stage4-evidence.tar.gz)에 보존했습니다.

객체 키와 구조 동등성·전체 custom default·typed constructor generator와 진단 표시 중 wrapper 실행은 후속 구현이 필요합니다. 함수/구조 타입·`Reference`·메타데이터·전체 진단·언어 코퍼스도 남아 있으므로 **M2 전체는 아직 완료되지 않았습니다**. 전체 gate의 1,950개 실패 이유와 미구현 960개·미연결 40개 accounting row를 유지합니다. 이 수치는 중복되는 요구사항·후보 기록이며 독립적인 기능 개수나 완료율이 아닙니다. 성능·메모리 목표에 대한 새 측정이나 달성 주장은 하지 않습니다.

네 번째 단계 구현의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39403541822) 작업 6개가 모두 성공했습니다. [CI 증거 검증](reports/m2/ci-stage4-summary.json)은 원본 artifact ZIP 두 개의 GitHub 해시와 소스 스냅샷 675개(고유 파일 520개)를 정확한 구현 커밋과 대조했습니다. 원격 결과도 413개 일치·기존 실패 2개이며, 전체 M2 gate는 실패로 유지합니다.

다섯 번째 단계는 런타임·GC·평가 순서를 `gpt-6-astra / high`, 프런트엔드·수명주기 검토와 Java 차등 증거를 각각 `gpt-6.1-sol / high`에 배정했습니다. `Listing<T>`·`Mapping<K,V>`·`List<T>` 검사와 wrapper, 일부 inferred new·클래스 기본 값, 별칭과 union 오류 위치를 연결했습니다. 함수 인자에 검사 결과 wrapper를 저장하고 GC의 임시·성공 반환값 추적, Host 재진입의 제약 eager 상태, 제약식 노드 동일성을 수정했습니다. `3a70e03`에 구현을 커밋·푸시했습니다. 원래 415개 입력을 유지하며 145개 사례를 추가한 [최종 로컬 검증](reports/m2/stage5-final-summary.json)은 560개 중 558개가 원시 바이트로 일치했고 기존 `% 0` 차이와 순환 참조 oracle 오류를 유지했습니다. Metamorphic 관계 20개, Rust 172개·Python 171개 검사, 실제 Node·브라우저 각 26개 시나리오가 통과했습니다. 소스 스냅샷 627개와 후보 실행 파일이 수집 중 바뀌지 않았음을 확인했습니다. 진단 표시 도중 부모 캐시를 가져와 wrapper 검사를 실행하는 Java 동작과 그 과정의 Java 오류는 미선택 탐색 증거로 유지합니다. 새 개발 예외는 추가하지 않습니다.

다섯 번째 단계 구현의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39411357487) 작업 6개가 모두 성공했습니다. [CI 증거 검증](reports/m2/ci-stage5-summary.json)은 원본 artifact ZIP의 GitHub 해시와 소스 스냅샷 824개(고유 파일 667개)를 정확한 구현 커밋과 대조했습니다. 원격에서도 560개 중 558개 일치·기존 실패 2개, 관계 20개와 Node·브라우저 각 26개 시나리오를 확인했습니다. 전체 gate는 1,950개 실패 이유를 유지하므로 **M2 전체는 계속 진행 중**입니다.

여섯 번째 단계의 member predicate (`[[조건]]`) 구현은 `ca751a1`에 커밋·푸시했습니다. 런타임·평가 순서는 `gpt-6-astra / high`, parser·resolver·독립 수명주기 검토와 Java 차등 검증은 각각 `gpt-6.1-sol / high`에 배정했습니다. 부모 선언의 child-first 순회, 조건과 값의 교대 평가, 선언 owner 캐시, custom `this` 복원, 지연 replacement·amendment, 중첩 generator와 const 문맥을 검증했습니다. 기존 560개 입력을 유지하며 113개를 추가했습니다. [최종 로컬 검증](reports/m2/stage6-final-summary.json)에서 673개 중 671개가 Java와 원시 바이트로 일치했고, 기존 `% 0` 차이와 순환 참조 oracle 오류는 유지했습니다. Metamorphic 관계 23개, Rust 191개·Python 171개 검사, 실제 Node·브라우저 각각 29개 시나리오가 통과했습니다. 소스 스냅샷 744개와 후보 실행 파일의 수집 중 동일성을 확인했습니다. 미선택 탐색 사례 8개와 수정 이전 입력 6개를 별도 증거로 보존하며 새로운 개발 예외는 추가하지 않았습니다. 늘어난 Java 차등 코퍼스와 직전 CI 실행 시간을 고려해 M2 작업 제한 시간을 20분에서 30분으로 늘렸습니다. 호환 판정이나 사례별 timeout·기존 실패 예외는 바꾸지 않았습니다.

여섯 번째 단계 구현의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39417418349) 작업 6개가 모두 성공했습니다. [독립 CI 검증](reports/m2/ci-stage6-summary.json)은 원본 artifact ZIP의 GitHub 해시와 소스 스냅샷 945개(고유 파일 784개)를 정확한 구현 커밋 `ca751a1`과 대조했습니다. 원격에서도 673개 중 671개 일치·기존 실패 2개, 관계 23개와 Node·브라우저 각 29개 시나리오를 확인했습니다. 봉인된 증거 패키지의 오프라인 재검증도 통과했고 패키지 내용이 바뀌지 않았음을 확인했습니다. 전체 gate는 1,950개 실패 이유를 유지하며, 구조적 객체 키·동등성, 남은 타입·진단·`Reference`·전체 언어 코퍼스가 미완료이므로 **M2 전체는 계속 진행 중**입니다.

일곱 번째 단계는 구조적 객체·컬렉션 동등성과 객체 키를 구현했습니다. 런타임 continuation·GC·평가 순서는 `gpt-6-astra / high`, Java 차등 사례와 독립 frontend·수명주기 검토는 각각 `gpt-6.1-sol / high`에 배정했습니다. 일반 `==`의 숫자 변환과 중첩 값·키의 boxed 비교를 구분하며, 객체 종류별 선행 검사·deep force 순서, 클래스 동일성, 상속·중복·amendment, Host 중단·재진입·실패 재시도를 검증했습니다. 작은 선언 map의 지연 비교와 다섯 번째 멤버의 해시 준비, 상속 키·Mapping 길이의 강제 평가 순서도 연결했습니다. 상수 List 키의 조기 중복 검증, 음수 리터럴의 숫자 토큰 위치, 함수 arity 표시와 Java 진단 highlighter의 보간 공백 동작을 맞췄습니다. 진단 재생은 명시적인 순회 스택을 사용하며 깊이 20,000인 잘못된 입력도 검사했습니다.

구현 커밋 `90f7df4`를 푸시했습니다. 기존 673개 선택 정의·입력 바이트와 알려진 실패를 유지하고 고정 Java에서 원시 결과를 확보한 201개를 추가했습니다. [최종 로컬 검증](reports/m2/stage7-final-summary.json)에서 전체 874개 중 872개가 원시 바이트로 일치했고, 기존 정수 `% 0` 차이와 순환 참조 oracle 오류를 유지했습니다. Metamorphic 관계 26개, Rust 210개·Python 171개 검사, 실제 Node·Chromium 각 32개 시나리오의 개발 계약이 통과했습니다. 기존 Rust probe 1개는 일반 단위 검사에서 계속 ignored이며 별도 차등 adapter로 실행합니다. 공통 primitive 6,078개 중 Java와 일치하는 6,076개 및 기존 regex 실패 2개를 구분합니다. 소스 스냅샷 950개와 후보 실행 파일의 수집 중 동일성을 확인했습니다. 탐색 증거는 내용 파일 14,196개에 원래 실패·미선택·수정 전·반복 수집을 보존하며 새 개발 예외는 추가하지 않았습니다. 전체 해시 충돌 순서·캐시 map 전환·순환 키, 함수/구조 타입·`Reference`·메타데이터·전체 진단·언어 코퍼스는 남아 있으므로 **M2 전체는 진행 중**입니다. 전체 gate의 1,950개 실패 이유를 유지하며 성능·메모리 목표에 대한 새 측정이나 달성 주장은 하지 않습니다.

일곱 번째 단계 구현의 [원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39427102753) 작업 6개가 모두 성공했습니다. [독립 CI 검증](reports/m2/ci-stage7-summary.json)은 원본 artifact ZIP 두 개의 GitHub 해시와 소스 스냅샷 1,165개(고유 경로 990개)를 정확한 구현 커밋 `90f7df4`와 대조했습니다. 원격에서도 874개 중 872개 일치·기존 실패 2개, 관계 26개와 Node·브라우저 각 32개 시나리오를 확인했습니다. 두 차례 오프라인 재검증이 통과했고, 마지막 검증에서는 봉인된 패키지 내용과 판정·검증기 식별자가 유지됨을 확인했습니다. 전체 M2 완료 조건과 성능 목표는 계속 열려 있습니다.

여덟 번째 단계는 일반 클래스의 `default` 속성, 성공한 멤버 캐시 삽입 순서와 클래스 선언 순서에 따른 구조 비교를 보완했습니다. Dynamic·Listing·Mapping·typed 객체의 deep force를 마친 뒤 내부 키·값의 해시를 준비하도록 평가 단계를 분리했습니다. 엄격한 List의 순차적 원소 해시 규칙은 유지했습니다. 런타임은 `gpt-6-astra / high`, Java 차등 증거와 독립 수명주기 검토는 각각 `gpt-6.1-sol / high`에 배정했습니다.

원래 874개 정의·입력과 기존 실패를 유지하며 33개 사례와 6개 변형 관계를 추가했습니다. Rust 212개·Python 171개 검사, Wasm 빌드, 기존 874개 재실행, 실제 Node·Chromium 각 35개 시나리오의 개발 계약이 통과했습니다. Host 중단·재진입·GC·실패 재시도·종료를 포함한 수명주기 검사 10개도 통과했습니다. 구현 커밋 `5024708`을 푸시했고 [최종 로컬 검증](reports/m2/stage8-final-summary.json)에서 907개 중 905개가 원시 바이트로 일치했습니다. 기존 정수 `% 0` 차이와 순환 참조 oracle 오류는 유지했습니다. Metamorphic 관계 32개가 통과했고 소스 스냅샷 983개를 정확한 구현 커밋과 대조했습니다. 실제 해시 버킷·캐시 다섯 번째 삽입 전환·일부 local 캐시 및 중첩 `let` 진단 프레임 차이 10개는 탐색 실패로 보존하며 새 개발 예외를 추가하지 않았습니다.

[원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39435577349) 작업 6개가 모두 성공했습니다. [CI 증거 검증](reports/m2/ci-stage8-summary.json)은 원본 artifact ZIP 해시와 소스 스냅샷 1,198개(고유 경로 1,023개)를 구현 커밋에 대조했습니다. 두 차례 오프라인 검증이 통과했고 최종 봉인 패키지는 바뀌지 않았습니다. 전체 gate의 1,950개 실패 이유와 성능 목표는 계속 열려 있으므로 **M2 전체는 진행 중**입니다.

아홉 번째 단계는 구조 동등성 ID와 Java 정수 해시를 분리하고, 안정적인 해시의 선언 버킷과 캐시의 부분 게시·재해시·실패 재시도를 구현했습니다. 런타임 continuation은 `gpt-6-astra / high`, 고정 Java 관찰·차등 입력과 독립 map·수명주기 검토는 각각 `gpt-6.1-sol / high`에 배정했습니다. 다섯 번째 캐시 값의 선행 게시, 실패 뒤 숨겨진 슬롯과 중복 값, local 캐시의 비대칭 비교, zero-hash 재계산을 검증합니다. Mapping 길이는 별도의 HashSet 규칙으로 조상부터 키를 순차 처리하고 성공한 길이만 게시합니다. 중첩 `let`·타입 제약·wildcard·명시적 cast의 오류 프레임은 소스 메타데이터로 복원합니다.

기존 907개 정의·입력 바이트를 유지하며 152개 사례와 9개 변형 관계를 추가했습니다. 구현 커밋 `2a69a63`을 푸시했고 [최종 로컬 검증](reports/m2/stage9-final-summary.json)에서 1,059개 중 1,057개가 Java와 원시 바이트로 일치했습니다. 기존 정수 `% 0` 차이와 순환 참조 oracle 오류는 유지했습니다. 관계 41개, Rust 221개·Python 171개 검사, Wasm 빌드와 실제 Node·Chromium 각 39개 시나리오의 개발 계약이 통과했습니다. 최종 소스 스냅샷 1,137개를 구현 커밋과 대조했습니다. 마감 회귀 검사에서 발견한 상속 Mapping 길이 차이와 중단된 수집·수정 전 검사 결과를 별도 보존했고 수정된 후보로 전체 검증을 다시 수행했습니다. 기존 탐색 증거를 덮어쓰거나 개발 예외를 추가하지 않았습니다.

[원격 CI](https://github.com/luuvish/pkl-rs/actions/runs/39529055834) 작업 6개가 모두 성공했습니다. [CI 증거 검증](reports/m2/ci-stage9-summary.json)은 원본 ZIP 해시와 소스 스냅샷 1,357개(고유 경로 1,177개)를 정확한 구현 커밋에 대조했습니다. 두 차례 오프라인 검증이 통과했고 최종 봉인 패키지는 바뀌지 않았습니다. JVM identity hash·같은 캐시의 순환 재진입·순환 키·재귀 진단 압축과 나머지 M2 언어·타입·Reference·메타데이터 범위가 남아 있으므로 **M2 전체는 진행 중**입니다. 전체 gate의 1,950개 실패 이유와 성능·메모리 목표도 계속 열려 있습니다.

다음 구현은 [`pkl:ref`](external/pkl/stdlib/ref.pkl)의 최소 실행 경로와 공통 선언·타입 메타데이터를 우선 연결합니다. 읽기 전용 의존성 조사만 완료했으며 아래 기능의 구현·검증은 아직 수행하지 않았습니다.

1. `import "pkl:ref"` namespace와 qualified 이름 해석, 선언 identity·module owner·상속 property 타입을 보존하는 메타데이터를 연결합니다.
2. Reference의 domain·data·path·referent type과 합성 property/subscript 접근을 구현합니다. 접근 중 실제 referent 본문이 실행되지 않는지 확인합니다.
3. 타입 정규화와 domain/referent 타입 검사를 구분하고, 렌더링·동등성·해시를 공통 VM·GC·Host 수명주기에 연결합니다.
4. upstream `basic/reference.pkl`, `errors/reference1..27.pkl`, `api/reference.pkl`과 최소화한 Java 원시 비교를 연결합니다. 전체 reflection API 및 남은 언어 코퍼스의 완료 조건은 계속 유지합니다.

- 전체 문법, 이름 해석, IR·compiler, lazy evaluation, 객체·amendment·상속, closure, generator를 구현합니다.
- 타입·제약·generic·union, `Reference<Domain, Type>`, reflection에 필요한 런타임 메타데이터를 구현합니다. 다중 행 문자열의 continuation 등 0.32 기능을 목록에 연결합니다.
- 구조화된 진단과 논리적 호출 스택으로 구문·이름 해석·실행 단계의 오류 및 선택 순서를 재현합니다. hidden override와 eager/lazy 타입 검사도 검증합니다.
- 해당 upstream 코퍼스와 rev1에서 선별한 회귀 입력을 Java/Rust 차등 실행에 연결하고 property·metamorphic 테스트와 differential fuzzing을 시작합니다.

**산출물·종료 조건:** 언어 목록의 해당 기능이 구현과 테스트에 연결되고 값·오류·위치가 일치해야 합니다. 발견한 불일치는 회귀 사례에 포함하며 언어 범위의 필수 미구현·미실행·알려진 불일치가 없어야 합니다.

## M3 — 표준 라이브러리와 출력

- 고정된 Pkl 표준 라이브러리 소스를 활용하고 모든 external builtin과 공개 멤버를 구현합니다. 등록 목록과 공개 API 목록을 기계적으로 대조합니다.
- PCF, JSON, YAML, XML, Properties, Plist, JSONnet, protobuf, Pkl Binary를 구현합니다. `output.bytes`, `output.files`, converter와 `Reference`의 렌더링·binary 표현을 포함합니다.
- escaping, 숫자 표현, 순서, 공백·개행, 잘못된 값과 형식의 오류, reflection을 비교합니다. 생성 파일의 경로 집합과 각 파일 바이트도 검증합니다.

**산출물·종료 조건:** stdlib 공개 멤버와 builtin 누락이 없고, 각 renderer의 성공·오류·경계값 코퍼스와 binary 비교가 통과해야 합니다.

## M4 — 모듈·리소스·프로젝트·패키지

- import/read/glob, URI, 파일·HTTP·modulepath, external reader를 공통 Host 계약에 연결합니다.
- PklProject, 설정 우선순위, 의존성 해석, dependency notation, local dependency, lockfile·checksum·cache·package 생성을 구현합니다.
- HTTP custom headers, `Project.resolvedEvaluatorSettings`, external reader의 경로·작업 디렉터리 동작을 포함합니다. redirect·proxy·CA, 접근 허용 규칙과 root 제한, 손상된 cache와 실패 경로를 검증합니다.
- 네트워크·파일 fixture를 고정하고 캐시 유무, import graph, 취소·timeout·close에 따른 자원 회수를 비교합니다.

**산출물·종료 조건:** 실제 I/O·프로젝트·패키지 코퍼스가 Java와 일치하고 접근 제한·오류·외부 프로세스 수명주기가 검증되어야 합니다. 해당 Pantry 패키지의 개발 트리와 배포 형태 모두 실행해야 합니다.

## M5 — 제품 API와 도구

- Rust evaluator API를 제공하고 CLI의 `eval`, `repl`, `server`, `test`, `project`, `download-package`, `analyze`, `format`, `run`, `shell-completion`을 구현합니다. 전체 옵션·기본값·help·종료 코드·파일 출력을 대조합니다.
- REPL의 PTY 세션·입력 편집·상태 유지와 server protocol의 codec·callback·요청 순서·취소·종료를 검증합니다. `spec`·`minimal` test reporter도 포함합니다.
- formatter, pkldoc, Java/Kotlin codegen을 구현하고 생성 결과를 바이트 비교합니다.
- 기존 Java/Kotlin 공개 API를 유지하는 JVM adapter를 별도 산출물로 제공합니다. 클래스·메서드·공개 타입·예외·수명주기를 대조하고 실제 기존 client로 확인합니다. 평가는 Rust engine이 수행합니다.

**산출물·종료 조건:** 공개 API 목록의 누락이 없고 기존 호출·client·CLI transcript·REPL·server 세션 및 생성 도구 결과가 계약을 충족해야 합니다.

## M6 — Wasm·N-API 제품 완성

- M1에서 만든 adapter를 전체 공통 engine과 연결하고 실제 브라우저·Node에서 대상 코퍼스를 실행합니다.
- Wasm에는 VFS·비동기 loader 등 Host 기능을 공급합니다. 필요한 기능과 제공 여부를 명시하고, 필수 기능을 제공하지 않는 환경의 사례를 전체 호환 통과로 집계하지 않습니다.
- N-API에는 Node I/O, 비동기 평가·취소·close, 손실 없는 `BigInt`와 `Uint8Array` 전달을 구현합니다. JS와 Rust 사이의 변환도 결과와 성능 검증에 포함합니다.
- Native 라이브러리·CLI, Wasm 배포물, npm 패키지와 JVM adapter의 빌드·설치·실행 절차를 검증하고 지원 플랫폼을 확정합니다.

**산출물·종료 조건:** 배포 artifact를 통해 실제 호출한 결과·오류·수명주기가 일치하고 공통 코퍼스 실행 결과가 target별로 보고되어야 합니다. 문서의 지원 기능은 검증된 범위와 일치해야 합니다.

## M7 — 전체 릴리스 검증

- 기능·API·테스트 목록과 실행 결과를 대조하고 upstream, Pantry, 회귀, fuzzing 발견 사례를 모두 검증합니다.
- 지원 OS·아키텍처·브라우저·Node 조합에서 배포물을 실행합니다. 플랫폼별 경로·개행·터미널 동작은 같은 환경의 Java와 비교합니다.
- 반복 성공·실패, 고유 소스 증가, evaluator 생성·종료, 취소, 캐시 변경과 순환 객체를 포함한 장기 실행으로 메모리 회수를 확인합니다.
- 정확한 결과를 낸 필수 workload 전체에 대해 실행 시간·메모리 측정을 완료하고 원시 자료, 환경, 반복 횟수, 분포와 판정을 공개합니다.

**종료 조건:** 필수 기능·API 누락과 알려진 불일치가 없어야 합니다. 필수 미실행·미구현·환경 오류·판정 보류가 남아 있으면 릴리스 완료로 선언하지 않습니다. 배포물의 재현 절차와 목표 달성 증거를 함께 제공해야 합니다.

## 전 단계의 성능·메모리 검증

필수 workload마다 아래 두 조건을 모두 적용합니다. M0에서 Java 기준을 만들고 M1부터 구현 변화에 따라 측정합니다. M5·M6에서 완성되는 API의 실제 호출 비용도 기준선에 연결합니다.

```text
Rust execution time / Java execution time ≤ 1.00
Rust peak RSS / Java peak RSS ≤ 1.00
```

CLI는 프로세스 시작부터 출력 완료·종료까지, 상주 evaluator는 초기화·첫 요청·반복 요청을 구분합니다. REPL·server의 세션 생성과 장기 실행, cold/warm cache, 큰 import graph·출력·실패 경로도 필수 workload에 포함합니다. 공식 native-image CLI는 별도 비교 자료로 남깁니다.

Wasm은 다운로드·컴파일·초기화와 상주 요청을 분리하고, N-API는 JS↔Rust 변환·결과 전달을 포함합니다. Host 프로세스와 측정 범위를 사전에 고정하여 peak RSS를 비교합니다. JVM heap, Rust allocation, Wasm linear memory는 원인 분석용이며 서로 다른 지표를 직접 나누어 메모리 목표 달성을 주장하지 않습니다.

반복 측정의 지연 분포와 불확실성을 함께 보고합니다. 평균 개선으로 개별 필수 workload의 퇴행을 숨기지 않으며, 차이가 불분명하면 판정 보류로 남깁니다. 성능 개선 후에는 같은 입력의 결과 바이트를 다시 확인합니다.

## 작업 완료와 계획 갱신

작업 완료 기록에는 변경 범위, 기능·테스트 ID, oracle 출처, 실행 환경, 비교 결과와 재현 자료 위치를 포함합니다. 수행하지 않은 검사와 남은 문제를 구분하고, 성능에 영향을 주는 변경에는 해당 workload 결과를 첨부합니다.

현재 실행 가능한 초기 구성 검사는 다음과 같습니다.

```sh
cargo xtask verify
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked
```

`cargo xtask verify`는 참조 소스 고정을 확인합니다. 위 명령은 Java 차등 검증이나 제품 성능 측정을 수행하지 않습니다. 현재 Java 기록·재비교·초기 성능 측정 명령과 실행 범위는 [TESTING.md](TESTING.md)에 정리되어 있습니다. Rust 평가기 구현 후 실제 Java/Rust 비교를 연결합니다. 단순 문서 수정에는 링크·내용·형식 확인 등 변경에 맞는 검증을 수행합니다.

단계별 로드맵과 상세 실행 계획은 이 문서에서 함께 관리합니다. 증거가 생길 때 단계 상태와 다음 작업을 갱신하고, 단계의 범위나 완료 기준이 바뀌면 요약 표와 해당 상세 절을 함께 수정합니다. 설계 결정은 아키텍처에, 검증 계약 변경은 TESTING.md에 반영합니다. 기준 버전·필수 기능·성능 목표를 바꾸는 경우에는 INTENT.md까지 함께 갱신하고 목표 변경 이유를 명시합니다. 실패를 없애기 위한 범위 축소는 일반 구현 진척으로 기록하지 않습니다.
