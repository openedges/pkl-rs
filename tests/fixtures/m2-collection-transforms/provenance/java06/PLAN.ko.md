# 빈 collection identity의 실제 공개 진단 관찰

기존01–05 및 설치된1272fixture는 변경하지 않는다. C의 source-only 최소 observer를 그대로 복사한 case와 List/Set/Map 변환 matrix를 새 Java root에서 실제 실행한다. PCF renderDocument가 strict collection root를 거부할 때 AbstractRenderer의 original/converted reference 비교에 따른 hint 유무를 전체 오류 bytes로 관찰한다. 어떤 heap layout이나 예상 hint를 미리 기댓값으로 만들지 않는다.

empty literal·receiver identity method·empty map/mapIndexed/mapNonNull 및 nonempty 입력에서 전부 null을 제거한 결과, List.toSet/Set.toList/Map.keys/values를 포함한다. nonempty 동일 receiver와 다른 AST에서 같은 내용을 만든 결과도 대조한다. 일부 mapNonNull converter에는 실제 trace를 넣어 callback 순서를 보존한다. helper는 기존 public evaluateExpressionString/ModuleSource.path profile, power assertions=false, logger/rawUTF16+JavaUTF8, explicit System.gc 요청을 사용한다. 원본 SourceCode adapter를 재실행하지 않는다. 모든 diagnostics·source·process identity를 보존하고 parser/setup 또는 다른 첫 오류면 의도 미도달로 남긴다. stdlib 전체나 canonical heap object layout 동일성을 주장하지 않는다.
