// Topology triple checker — same / reflection.

module topology_unit (
    input  wire [1:0] a,
    input  wire [1:0] b,
    input  wire [1:0] c,
    output wire [1:0] check_L,
    output wire [1:0] check_R,
    output wire [1:0] status
);
    wire [1:0] compose_ab, compose_bc, compose_abc, compose_abc2;
    wire [1:0] dummy_c, reflected_R;
    wire [1:0] r1, r2, r3, r4;
    wire s1, s2, s3, s4, s5, e1, e2, e3, e4, e5;

    phase_alu alu1(.a(a), .b(b), .compose_out(compose_ab), .reflect_out(r1), .t_sign_out(s1), .t_energy_out(e1));
    phase_alu alu2(.a(compose_ab), .b(c), .compose_out(compose_abc), .reflect_out(r2), .t_sign_out(s2), .t_energy_out(e2));
    phase_alu alu3(.a(b), .b(c), .compose_out(compose_bc), .reflect_out(r3), .t_sign_out(s3), .t_energy_out(e3));
    phase_alu alu4(.a(a), .b(compose_bc), .compose_out(compose_abc2), .reflect_out(r4), .t_sign_out(s4), .t_energy_out(e4));
    phase_alu refl(.a(compose_abc2), .b(2'd0), .compose_out(dummy_c), .reflect_out(reflected_R), .t_sign_out(s5), .t_energy_out(e5));

    assign check_L = compose_abc;
    assign check_R = compose_abc2;
    assign status = (check_L == check_R) ? 2'd0 : ((check_L == reflected_R) ? 2'd1 : 2'd2);
endmodule
