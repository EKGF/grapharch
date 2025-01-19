a:
  @echo 'A!'

b: a && c d
  @echo 'B!'

[no-exit-message]
c:
  @echo 'C!'
  exit 1

d:
  @echo 'D!'

e: b
  @echo 'E!'
