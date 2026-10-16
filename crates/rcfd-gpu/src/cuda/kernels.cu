// f64 kernels for the device linear solvers (ADR 0038 D5). Compiled at run time by NVRTC with --fmad=false so the
// elementwise kernels and the SpMV follow the two-rounding arithmetic of the host code.
#define RB 256

extern "C" {

// y = A x, one thread per row, sequential accumulation in row order (same order as the host matvec)
__global__ void spmv(const int n, const int* __restrict__ rp, const int* __restrict__ ci,
                     const double* __restrict__ v, const double* __restrict__ x, double* __restrict__ y) {
    const int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) {
        double s = 0.0;
        const int e = rp[i + 1];
        for (int k = rp[i]; k < e; ++k) {
            s += v[k] * x[ci[k]];
        }
        y[i] = s;
    }
}

// y += a x
__global__ void axpy(const int n, const double a, const double* __restrict__ x, double* __restrict__ y) {
    const int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) y[i] += a * x[i];
}

// y = x + beta y
__global__ void xpby(const int n, const double beta, const double* __restrict__ x, double* __restrict__ y) {
    const int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) y[i] = x[i] + beta * y[i];
}

// r = b - r
__global__ void rsub(const int n, const double* __restrict__ b, double* __restrict__ r) {
    const int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) r[i] = b[i] - r[i];
}

// out = a * b (diagonal preconditioner)
__global__ void mul(const int n, const double* __restrict__ a, const double* __restrict__ b, double* __restrict__ out) {
    const int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = a[i] * b[i];
}

// BiCGStab direction: p = r + beta (p - omega v)
__global__ void bicg_p(const int n, const double beta, const double omega, const double* __restrict__ r,
                       const double* __restrict__ v, double* __restrict__ p) {
    const int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) p[i] = r[i] + beta * (p[i] - omega * v[i]);
}

// out = a - alpha b
__global__ void sub_scaled(const int n, const double alpha, const double* __restrict__ a,
                           const double* __restrict__ b, double* __restrict__ out) {
    const int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) out[i] = a[i] - alpha * b[i];
}

// Chebyshev start: rc = dinv r, d = rc / theta
__global__ void cheb_init(const int n, const double theta, const double* __restrict__ dinv,
                          const double* __restrict__ r, double* __restrict__ rc, double* __restrict__ d) {
    const int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) {
        const double q = dinv[i] * r[i];
        rc[i] = q;
        d[i] = q / theta;
    }
}

// Chebyshev step: rc -= dinv t, d = c1 d + c2 rc
__global__ void cheb_step(const int n, const double c1, const double c2, const double* __restrict__ dinv,
                          const double* __restrict__ t, double* __restrict__ rc, double* __restrict__ d) {
    const int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i < n) {
        const double q = rc[i] - dinv[i] * t[i];
        rc[i] = q;
        d[i] = c1 * d[i] + c2 * q;
    }
}

// Block partial sums of a.b (fixed grid, grid-stride accumulation, fixed-shape tree: order depends only on n)
__global__ void dot_partial(const int n, const double* __restrict__ a, const double* __restrict__ b,
                            double* __restrict__ part) {
    __shared__ double sh[RB];
    const int t = threadIdx.x;
    double s = 0.0;
    const long long stride = (long long)gridDim.x * RB;
    for (long long i = (long long)blockIdx.x * RB + t; i < n; i += stride) s += a[i] * b[i];
    sh[t] = s;
    __syncthreads();
    for (int o = RB / 2; o > 0; o >>= 1) {
        if (t < o) sh[t] += sh[t + o];
        __syncthreads();
    }
    if (t == 0) part[blockIdx.x] = sh[0];
}

// Two dot products at once: part[0..g) = a.b, part[g..2g) = c.d
__global__ void dot2_partial(const int n, const double* __restrict__ a, const double* __restrict__ b,
                             const double* __restrict__ c, const double* __restrict__ d, double* __restrict__ part) {
    __shared__ double sh[RB];
    const int t = threadIdx.x;
    const long long stride = (long long)gridDim.x * RB;
    double s = 0.0;
    for (long long i = (long long)blockIdx.x * RB + t; i < n; i += stride) s += a[i] * b[i];
    sh[t] = s;
    __syncthreads();
    for (int o = RB / 2; o > 0; o >>= 1) {
        if (t < o) sh[t] += sh[t + o];
        __syncthreads();
    }
    if (t == 0) part[blockIdx.x] = sh[0];
    __syncthreads();
    s = 0.0;
    for (long long i = (long long)blockIdx.x * RB + t; i < n; i += stride) s += c[i] * d[i];
    sh[t] = s;
    __syncthreads();
    for (int o = RB / 2; o > 0; o >>= 1) {
        if (t < o) sh[t] += sh[t + o];
        __syncthreads();
    }
    if (t == 0) part[gridDim.x + blockIdx.x] = sh[0];
}

// Block partial sums of a
__global__ void sum_partial(const int n, const double* __restrict__ a, double* __restrict__ part) {
    __shared__ double sh[RB];
    const int t = threadIdx.x;
    double s = 0.0;
    const long long stride = (long long)gridDim.x * RB;
    for (long long i = (long long)blockIdx.x * RB + t; i < n; i += stride) s += a[i];
    sh[t] = s;
    __syncthreads();
    for (int o = RB / 2; o > 0; o >>= 1) {
        if (t < o) sh[t] += sh[t + o];
        __syncthreads();
    }
    if (t == 0) part[blockIdx.x] = sh[0];
}

// One block: out[s] = sum of part[s*m .. s*m+m) for s < k, fixed order
__global__ void reduce_final(const int m, const int k, const double* __restrict__ part, double* __restrict__ out) {
    __shared__ double sh[RB];
    const int t = threadIdx.x;
    for (int s = 0; s < k; ++s) {
        double acc = 0.0;
        for (int i = t; i < m; i += RB) acc += part[s * m + i];
        sh[t] = acc;
        __syncthreads();
        for (int o = RB / 2; o > 0; o >>= 1) {
            if (t < o) sh[t] += sh[t + o];
            __syncthreads();
        }
        if (t == 0) out[s] = sh[0];
        __syncthreads();
    }
}

}
