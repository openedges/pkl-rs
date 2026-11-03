# Stage141 유한 publication 순차 실행

실제 20개 part와 각 독립 readback, 마지막 `verify-parts.py`를 순서대로 실행한다. 각 subprocess의 명령·종료 코드·stdout/stderr와 산출 receipt를 보존하고, 실패 즉시 중단하며 기존 경로를 재사용하지 않는다. publisher는 각 archive의 50 MiB 미만 제한과 1 GiB 디스크 floor를 검사한다. 분할 index와 selection, helper 및 HEAD는 전후 hash로 묶는다.

root가 native 작업 종료와 저장소 조건을 확인하고 source review를 마친 뒤에만 실행한다.

```sh
python3 -B target/m2/stage141-publication-sequence-prep-01/run.py
```

archive 완성과 독립 읽기는 최종 게시 사본 검증·정확한 Git blob 확인이나 전체 실행 환경 복원을 대신하지 않는다. 실패한 부분 결과는 보존하고 새 승인된 output만 사용한다.
