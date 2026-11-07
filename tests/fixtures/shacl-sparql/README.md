# SHACL SPARQL formatting regression

`dprod-schedule.ttl` retains the prefix declarations and the unchanged
`dprod-shapes:ScheduleShape` assertion from
[EKGF/dprod at 95796fb841d6f168c6878768ffa085b166d3b175](https://github.com/EKGF/dprod/blob/95796fb841d6f168c6878768ffa085b166d3b175/dprod-contracts/dprod-contracts-shapes.ttl#L321).
It is a small offline regression excerpt, not a published documentation source.
The source ontology declares the
[Creative Commons Attribution 4.0 license](https://creativecommons.org/licenses/by/4.0/).
Attribution: EKGF / Object Management Group, DPROD Data Contracts shapes.

The original multiline queries exercise nested `sh:sparql` blank nodes and a
long regular-expression line. Tests add a separate original `sh:ask` example.
