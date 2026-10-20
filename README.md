# pkl-rs

[Pkl](https://pkl-lang.org/) 0.32.0을 Rust로 새로 구현하는 프로젝트입니다. 공통 엔진을 Native Rust, WebAssembly, Node.js N-API에서 사용하고, Java 구현과의 호환성을 자동으로 검증하는 것을 목표로 합니다.

현재는 **저장소와 개발 환경을 구성한 초기 단계**입니다. Pkl 파싱·평가, CLI·REPL·server, Wasm·N-API 제품은 아직 구현되지 않았습니다. 현재 빌드와 테스트의 성공은 Pkl 호환성이나 성능 목표의 달성을 의미하지 않습니다.

## 목표

- Pkl 0.32.0의 언어, 표준 라이브러리, CLI, REPL, server 및 공개 API 호환성.
- 동일 입력과 환경에서 출력 바이트, 생성 파일, 오류 메시지, 소스 위치, 종료 코드 일치.
- 필수 워크로드별 실행 시간과 peak RSS가 Java 기준 이하.
- 하나의 Rust 평가 엔진을 사용하는 Native, Wasm, N-API 지원.
- upstream·Pantry·회귀·차등·성능 테스트로 검증 범위와 미구현 항목 추적.

완료 기준과 검증 방식은 [TESTING.md](TESTING.md), 구현 방향은 [아키텍처](docs/architecture.md)와 [로드맵](docs/roadmap.md)에 정리합니다.

## 참조 버전

참조 소스는 `external/`의 Git submodule로 고정합니다. Git gitlink와 `references.lock`이 같은 commit을 가리켜야 합니다.

| 경로 | 용도 | 고정 commit |
| --- | --- | --- |
| `external/pkl` | Pkl 0.32.0의 언어·API·표준 라이브러리·테스트 기준 | `e51fb2f91774265666f94980ec630db794a4db2d` |
| `external/pkl-pantry` | 실제 패키지·예제를 이용한 호환성 코퍼스 후보 | `904a68184e54da378b0a0e8a911a3fab3956f693` |

Pantry의 고정 revision은 코퍼스 입력 기준입니다. 전체 코퍼스를 Java 0.32.0 또는 Rust로 실행해 검증한 상태는 아닙니다. `../pkl-rs.rev1`은 설계·테스트 사례를 참고하는 이전 작업이며, 빌드 의존성으로 사용하지 않습니다.

## 시작하기

Git과 [rustup](https://rustup.rs/)이 필요합니다. Rust 버전은 `rust-toolchain.toml`에 고정하며, rustup이 지정된 도구를 설치합니다. 아래 URL은 실제 저장소 주소로 바꿉니다.

```sh
git -c core.autocrlf=false clone --recurse-submodules <repository-url> pkl-rs
cd pkl-rs
git config --local core.autocrlf false
git submodule foreach --recursive 'git config --local core.autocrlf false'
```

이미 clone했다면 참조 소스를 초기화합니다.

```sh
git -c core.autocrlf=false submodule update --init --recursive
git config --local core.autocrlf false
git submodule foreach --recursive 'git config --local core.autocrlf false'
```

`core.autocrlf=false`는 참조 fixture가 Windows에서 자동으로 줄바꿈 변환되는 것을 막습니다. 각 upstream의 명시적 `.gitattributes`는 유지됩니다. 이미 변환된 checkout이 있다면 기존 변경을 보존한 뒤 위 방식으로 새로 clone합니다.

초기 구성의 검증 명령은 다음과 같습니다.

```sh
cargo xtask verify
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check -p pkl-runtime --target wasm32-unknown-unknown --locked
```

`cargo xtask verify`는 참조 소스의 버전 고정을 확인합니다. Pkl 평가 결과를 Java와 비교하는 명령은 후속 단계에서 구현합니다.

## 현재 구조

```text
.cargo/                 Cargo 별칭과 공통 설정
.github/workflows/      초기 구성 검증 CI
crates/pkl-runtime/     공통 엔진을 위한 최소 Rust 라이브러리
docs/                   아키텍처와 단계별 계획
external/pkl/           Pkl 0.32.0 참조 submodule
external/pkl-pantry/    Pantry 참조 submodule
xtask/                  저장소 검증 도구
references.lock         참조 저장소와 revision 기록
rust-toolchain.toml     Rust 도구 버전
```

기여 절차는 [CONTRIBUTING.md](CONTRIBUTING.md), 저장소에서 작업하는 에이전트의 규칙은 [AGENTS.md](AGENTS.md)를 따릅니다. 본 프로젝트는 [Apache License 2.0](LICENSE)을 사용합니다. 참조 저장소의 라이선스는 각 submodule에 포함된 원문을 확인합니다.
