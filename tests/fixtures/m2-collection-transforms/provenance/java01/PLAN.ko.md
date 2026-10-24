# Stage51 Collection map 계열 Java-first 관찰

Stage50 runtime/source/fixture/Git을 변경하지 않고 이 새 target root에만 입력과 도구·원시 결과를 작성한다. 기준은 pinned Pkl 0.32.0 원본 JAR, Temurin21.0.12.1+1-LTS다. 원본 SourceCode fixture의 전체 source bytes와 `result` 식은 그대로 복사하며 원래 M2LanguageProbe adapter 2회와 새 sequential collector를 구분한다. 새 collector는 evaluateExpressionString의 stdout/stderr/status/error class·message-null, Java UTF8 및 UTF16BE를 독립 보존한다.

15 cases ×2 fresh JVM repeats ×2 같은 JVM의 새 evaluator sessions, 합계940 예정 API calls다. List/Set의 map/mapIndexed/mapNonNull/mapNonNullIndexed, Map의 map/mapKeys/mapValues에 대해 빈 입력·Function0–3/nonfunction guard, typed callback parameter/return, callback 순서와 첫 오류, Pair 계약, output 중복/대표 key, structural hash forcing, lazy 결과와 reader retry/guest catch를 조사한다. `Null(new Dynamic {...})`는 pinned base Null 함수의 실제 null-with-default 구문이다. GC는 System.gc 요청이며 완료/수명 증거가 아니다. close 뒤 API 예외는 별도 boundary로 남긴다.

Logger는 실제 trace/warn 메시지와 StackFrame의 source URI/member-null/좌표/source-lines를 그대로 기록한다. ct reader는 입력 TSV의 정확 URI/결과만 사용한다. io-first의 첫 실제 read IOException과 성공 Text ResourceManager cache를 구분한다. Host에서 map/dedup/URI 정규화/semantic cache를 구현하지 않는다. callback→output key hash/force→다음 callback은 실제 trace로 관찰한다. Map iterator의 key lookup은 pinned 소스에 있으나 immutable key cache 때문에 각 lookup이 별도 관측 이벤트로 드러나지 않을 수 있어 source proof와 분리한다.

모든 group의 첫/마지막 stable 성공과 실패 진단을 직접 읽어 실제 의도 도달 여부를 후속 감사한다. parser/fixture/collector 초기 실패가 있으면 원문을 보존하고 새 root에서 수정한다. Java 기대 결과를 Rust로 생성하지 않는다. 원본 api/list.pkl·set.pkl·map.pkl 전체/표준 라이브러리 전체 성공이나 Rust/native/browser 완료를 주장하지 않는다. Rust per-poll GC, reentry, cancellation/timeout, stale token 및 code-owner 회귀는 별도 의무다.
