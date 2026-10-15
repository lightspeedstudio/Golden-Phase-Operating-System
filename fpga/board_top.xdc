# KairOS FPGA constraints (example — Artix-7 style)
set_property PACKAGE_PIN W5 [get_ports clk]
set_property PACKAGE_PIN U18 [get_ports rst_n]
set_property PACKAGE_PIN B18 [get_ports uart_tx]
set_property PACKAGE_PIN E17 [get_ports heartbeat]
set_property IOSTANDARD LVCMOS33 [get_ports *]
create_clock -period 10.000 -name sys_clk [get_ports clk]
