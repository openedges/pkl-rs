# Stage141 게시 사본 준비

실제 20개 publisher·독립 reader·통합 verify-parts·41단계 sequence가 모두 성공한 뒤에만 root가 실행한다. 새 `reports/m2/stage141-jsonnet-xml`에 정확한 20개 numbered archive, part selection·publisher/readback·members·input bookend, partition·aggregate·sequence receipt, runner source/plan 및 41단계 stdout/stderr를 복사한다. source와 사본의 SHA·크기·mode를 전후 확인하고 `publication.json`에 대응 관계를 기록한다. source path 및 옛 archive는 순회하지 않는다.

```sh
python3 -B target/m2/stage141-publication-copy-prep-01/copy.py
```

실패하면 부분 사본과 receipt를 보존한다. archive 사본, 후속 문서 및 정확한 Git staged/committed blob은 별도로 검증해야 한다. 전체 compiler/cache/source 복원을 주장하지 않는다.
