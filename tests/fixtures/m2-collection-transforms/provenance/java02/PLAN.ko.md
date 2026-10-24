# Stage51 Java01 의도 미도달 경계의 새 입력 교정

원래01의 940 API와 초기 parser24·bare method-reference16 오류·post-close12 API 경계는 그대로 보존한다. 이 새 root는 bare method 값을 넘기지 않고 `(x)->typed(x)`/`bad(x)`/`(k,v)->badPair(k,v)`의 실제 호출을 사용하고, pkl:test import를 선언 앞에 둔다. 직접 typed lambda의 빈/비어 있지 않은 입력과 Set/Map callback 첫·중간·마지막 throw를 추가한다. 3 cases·88 API를 새 Java JVM 6개와 각2 evaluator session으로 관찰한다. 원본 SourceCode adapter는 봉인01에서 재사용하며 다시 실행하지 않는다.

collector와 raw API/UTF16/JavaUTF8/logger/read 계약은01과 동일하다. source/query bytes는02에서만 새로 작성한다. Java 기대 결과를 Rust로 만들지 않고,01의 실패를 성공으로 바꾸지 않는다. complete stdlib/수명/재진입/플랫폼 호환성의 완료 주장은 없다.
