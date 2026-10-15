# KairOS FPGA — simulation + iCEBreaker (UP5K) flash

SRCS = phase_alu.v phase_registers.v phase_decoder.v \
	topology_unit.v phase_memory.v scheduler_unit.v memory_unit.v \
	trace_bus.v phase_cpu.v uart_tx.v board_top.v

ICE_SRCS = phase_alu.v phase_registers.v phase_decoder.v \
	topology_unit.v trace_bus.v phase_cpu.v uart_tx.v icebreaker_top.v

.PHONY: sim icebreaker prog clean

sim:
	iverilog -g2012 -o sim $(SRCS) testbench.sv
	vvp sim

# Build icebreaker.bin (requires OSS CAD Suite on PATH)
icebreaker: icebreaker.bin

icebreaker.json: $(ICE_SRCS)
	yosys -p "read_verilog $(ICE_SRCS); synth_ice40 -top icebreaker_top -json $@"

icebreaker.asc: icebreaker.json icebreaker.pcf
	nextpnr-ice40 --up5k --package sg48 --json icebreaker.json --pcf icebreaker.pcf --asc $@

icebreaker.bin: icebreaker.asc
	icepack $< $@

prog: icebreaker.bin
	iceprog icebreaker.bin

clean:
	rm -f sim icebreaker.json icebreaker.asc icebreaker.bin *.vcd
