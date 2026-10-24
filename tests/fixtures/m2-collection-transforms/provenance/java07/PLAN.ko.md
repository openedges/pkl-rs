# Duration 기초 value/형식/hash/identity Java-first 추가 관찰

원래1272 corpus와06은 변경하지 않는다. A/root가 승인한 별도07은 Int/Float 7개 unit 생성(리터럴 및 동적 getter), 큰 Int widening·i64경계·분수·signed zero·NaN/Infinity, value/unit/isPositive/getClass/타입, interpolation·기본PCF·Null union guard, nanosecond equality 및 Set/Map의 hash/첫대표/최종value, map 변환/reader retry를 실제 Java로 고정한다. unary Duration 부호는 자연스러운 음수 구문의 별도 작은 관찰이며 다른 arithmetic·toUnit·ISO·DataSize는 미실행으로 남긴다.

AstBuilder의 literal-unit folding과 동적 getter를 구분한다. 같은 literal 함수/nullable literal closure를 2회 호출한 뒤 PCF conversion hint observer를 사용하고, 다른 AST·동적 x.s/x?.s·tag가 다른 두 객체의 literal/계산 property를 대조한다. heap layout이나 예상 identity/hint/equality를 사전 기댓값으로 만들지 않는다. Java source/runtime/artifact·전체 raw status/UTF16/JavaUTF8/logger/read·실제 PID/terminal을 보존하며 기존 collector(power=false)를 그대로 사용한다. 원본SourceCode adapter는 재실행하지 않는다. 각group stable과 첫오류를 읽어 의도도달/셋업실패를 분리한다.
