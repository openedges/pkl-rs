# Null default amendment의 정확한 parenthesized 입력

04의 36 parser 실패는 그대로 보존한다. 실제 진단의 parenthesized amends 요구에 따라 새05 source에서 `(defaultObject) {...}`/`(defaultFunction) {...}`를 사용한다. Function default amendment가 함수 값을 반환하는지와 `.apply` 뒤 객체를 읽는 경계, Function1 인수 count, Object default의 비강제 및 mapNonNull 제거를 독립 관찰한다. 잘못된 Function property/arity 조회는 의도한 ordinary error control로 남긴다. 원래1060 corpus 및04raw를 변경하지 않는다. 56 API, 새 JVM2개/각 evaluator2sessions, Java-only이며 Rust 기대를 생성하지 않는다.
