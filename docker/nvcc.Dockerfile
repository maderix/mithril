# The CUDA compiler Mithril's GPU backend runs: generated CUDA is compiled with
# `nvcc` inside this image (the host needs no CUDA toolkit, only the driver).
# Same compiler build as Blaze's blaze-ptx:cu13x image (CUDA 13.0.1, nvcc
# V13.0.88), without Blaze's LLVM and MLIR.
#
#   docker build -f docker/nvcc.Dockerfile -t mithril-nvcc:cu13.0 .
#
# Another image can be used with MITHRIL_NVCC_IMAGE=<image>.
FROM nvidia/cuda:13.0.1-devel-ubuntu24.04@sha256:7d2f6a8c2071d911524f95061a0db363e24d27aa51ec831fcccf9e76eb72bc92

WORKDIR /w
CMD ["nvcc", "--version"]
