# 모듈 소유 callback catch의 최소 실제 관찰

01의 import 위치 오류24개, bare method 값 오류16개와02의 API repl에서 module-local import alias를 찾지 못한16개를 모두 보존한다. 새03에서는 pkl:test import를 소유한 모듈의 실제 속성에 catch 계산을 작성하고 public API는 그 속성을 조회한다. 따라서 source owner·함수 frame·쿼리가 새로 달라진다는 점을 명시한다. 내부 catch가 null을 반환해 map을 계속하는 경우, 외부 catch가 실패한 transform을 unwind하는 경우, 성공한 속성의 재조회와 System.gc 요청 뒤 stable을 32 API에서 관찰한다. 원본 SourceCode adapter·이미 도달한01/02 일반 cases는 재실행하지 않는다. Java만 실제취득하며 Rust 기대 생성·production/Git 변경은 없다.
