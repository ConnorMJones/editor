# Math

LaTeX math, rendered through typst. Inline math sits in the text, such as
$e^{i\pi} + 1 = 0$, $\alpha + \beta \leq \gamma$ or $\frac{a}{b}$.

## Fractions and sums

$$
\sum_{k=1}^{n} k = \frac{n(n+1)}{2}
$$

$$
\prod_{i=1}^{n} x_i \geq \left( \frac{1}{n} \sum_{i=1}^{n} x_i \right)^{n}
$$

## Integrals

$$
\int_{-\infty}^{\infty} e^{-x^2} \, dx = \sqrt{\pi}
$$

$$
\oint_C \mathbf{F} \cdot d\mathbf{r} = \iint_S (\nabla \times \mathbf{F}) \cdot d\mathbf{S}
$$

## Matrices

$$
A = \begin{pmatrix} a & b \\ c & d \end{pmatrix}, \quad
\det A = ad - bc
$$

$$
\begin{bmatrix} 1 & 0 & 0 \\ 0 & \cos\theta & -\sin\theta \\ 0 & \sin\theta & \cos\theta \end{bmatrix}
$$

## Aligned equations

$$
\begin{aligned}
(a + b)^2 &= (a + b)(a + b) \\
          &= a^2 + 2ab + b^2
\end{aligned}
$$

## Cases

$$
|x| = \begin{cases} x & \text{if } x \geq 0 \\ -x & \text{otherwise} \end{cases}
$$

## Greek letters

$\alpha, \beta, \gamma, \delta, \epsilon, \lambda, \mu, \pi, \sigma, \phi, \omega$
and $\Gamma, \Delta, \Lambda, \Sigma, \Phi, \Omega$.

Back to the [[index]].
