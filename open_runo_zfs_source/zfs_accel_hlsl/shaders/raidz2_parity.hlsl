// RAID-Z2 (二重パリティ P/Q) 用 Reed-Solomon Compute Shader
//
// GF(2^8)(既約多項式 x^8+x^4+x^3+x^2+1 = 0x11d、生成元2)上で
// P = XOR(D_i)、Q = XOR(D_i * 2^i) を計算する。
// 対応するCPU参照実装・テストは ../src/raidz23_parity.rs, ../src/galois.rs を参照。
//
// RAID-Z3のR(4^i係数)は本シェーダのQ計算ロジックを4回の二乗(=2^2i倍)で
// 流用できるが、現段階では未実装(TODO)。

RWStructuredBuffer<uint> DataStripes : register(u0); // [num_disks * stripe_len_words]
RWStructuredBuffer<uint> ParityPOut  : register(u1); // [stripe_len_words]
RWStructuredBuffer<uint> ParityQOut  : register(u2); // [stripe_len_words]

cbuffer Params : register(b0)
{
    uint NumDisks;
    uint StripeLenWords;
    uint Reserved0;
    uint Reserved1;
};

// GF(2^8)上で1バイトを2倍する(既約多項式0x11dの下位8bit=0x1dで還元)。
uint gf_mul2_byte(uint b)
{
    uint shifted = (b << 1) & 0xFFu;
    uint hibit = (b >> 7) & 1u;
    return shifted ^ (hibit * 0x1Du);
}

// 32bit語にパックされた4バイトそれぞれをGF(2^8)上で独立に2倍する。
uint gf_mul2_packed(uint v)
{
    uint b0 = gf_mul2_byte(v & 0xFFu);
    uint b1 = gf_mul2_byte((v >> 8) & 0xFFu);
    uint b2 = gf_mul2_byte((v >> 16) & 0xFFu);
    uint b3 = gf_mul2_byte((v >> 24) & 0xFFu);
    return b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);
}

[numthreads(256, 1, 1)]
void CSMain(uint3 dtid : SV_DispatchThreadID)
{
    uint word_idx = dtid.x;
    if (word_idx >= StripeLenWords)
    {
        return;
    }

    uint p_acc = 0;
    uint q_acc = 0;

    for (uint disk = 0; disk < NumDisks; ++disk)
    {
        uint word = DataStripes[disk * StripeLenWords + word_idx];
        p_acc ^= word;

        // term = word * 2^disk (各バイトレーンを独立にdisk回2倍する)
        uint term = word;
        for (uint k = 0; k < disk; ++k)
        {
            term = gf_mul2_packed(term);
        }
        q_acc ^= term;
    }

    ParityPOut[word_idx] = p_acc;
    ParityQOut[word_idx] = q_acc;
}
