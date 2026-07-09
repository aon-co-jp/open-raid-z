// RAID-Z1 (単一パリティ) 用XOR計算 Compute Shader — NPU専用ディスパッチ経路。
//
// 【現状】アルゴリズムは shaders/raidz_parity.hlsl (GPU用) と完全に同一。
// NPU実機が無く、DirectMLの行列演算ユニットを活かした専用実装(Tensor
// Op経由等)が実際に高速化に寄与するか検証できないため、まずは正しさが
// 保証されているGPU版と同じロジックをNPU経路として分離しておく
// (`AccelKind::Npu`は`AccelKind::Gpu`と別のシェーダバイトコードを使う、
// `src/raidz_parity.rs`/`src/compute.rs`参照)。これにより、将来NPU実機で
// 専用最適化(DirectML Tensor演算への置き換え等)を行っても、GPU側の
// 検証済みパスに影響しない。
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
