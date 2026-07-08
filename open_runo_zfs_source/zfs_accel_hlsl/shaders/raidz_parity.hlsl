// RAID-Z1 (単一パリティ) 用XOR計算 Compute Shader
// 実運用のRAID-Z2/Z3はGF(2^8)上のReed-Solomon演算が必要で、
// これは別途 raidz_parity_rs.hlsl (未実装) で対応する想定。
//
// 入力: N個のデータストライプ(各32bit単位でパック)
// 出力: XORパリティストライプ

RWStructuredBuffer<uint> DataStripes : register(u0); // [num_disks * stripe_len_words]
RWStructuredBuffer<uint> ParityOut   : register(u1); // [stripe_len_words]

cbuffer Params : register(b0)
{
    uint NumDisks;
    uint StripeLenWords;
    uint Reserved0;
    uint Reserved1;
};

[numthreads(256, 1, 1)]
void CSMain(uint3 dtid : SV_DispatchThreadID)
{
    uint word_idx = dtid.x;
    if (word_idx >= StripeLenWords)
    {
        return;
    }

    uint acc = 0;
    for (uint disk = 0; disk < NumDisks; ++disk)
    {
        acc ^= DataStripes[disk * StripeLenWords + word_idx];
    }

    ParityOut[word_idx] = acc;
}
