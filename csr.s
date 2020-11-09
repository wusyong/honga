main:
  addi t0, zero, 1
  addi t1, zero, 2
  addi t2, zero, 3
  csrrw zero, sstatus, t0
  csrrs zero, stvec, t1
  csrrw zero, sepc, t2
  csrrc t2, sepc, zero
  csrrwi zero, mstatus, 1
  csrrsi zero, mtvec, 2
  csrrwi zero, mepc, 3
  csrrci t2, mepc, 0
