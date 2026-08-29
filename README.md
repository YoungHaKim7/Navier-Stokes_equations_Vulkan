# Navier-Stokes_equations_Vulkan
나비에-스토크스 방정식(Navier-Stokes equations, NS equation)


# 공식(NS Equation)(ChatGPT)

The **Navier–Stokes (NS) equations** describe the motion of fluids such as water, air, and gases. They express **Newton's second law applied to a fluid**.

# 유튜브에 나온 공식들.

- [공식 9분 12초에 나온다.](https://youtu.be/egfRLw2yzng?si=661b5rze8GtUYn1_&t=552)


$$ \rho\left( \frac{\partial u_i}{\partial t} + u_j \frac{\partial u_i}{\partial x_j} \right) = -\frac{\partial p}{\partial x_i} + \mu\frac{\partial^2 u_i}{\partial x_j \partial x_j}  $$

- 맨 앞에 P와 닮은 그리스 문자 (로)`Rho` (`P`, `ρ`) 는 밀도를 말한다.

- P = 압력
  - 압력(P) 이 유체의 속도(u) 를 좌우한다.

$$
 \partial u_i
 \partial u_j
$$ 

- 속도(속도를 나타내는 표현들)(ui, uj 등등)

- 핵심 개념 3가지
```txt
1. 밀도(로우 ρ)
2. 점성(뮤 u)
3. 압력(P)
```

- 미는 힘
  - 압력(P)이 유체의 속도(u)를 좌우한다.
- 

# 1. Partial Differential / Particle Symbol (∂)
- This is the most common symbol requested under this phonetic spelling.
  - Unicode: `U+2202`
  - LaTeX: `\partial`
- Here is the quick reference for the mathematical symbol used to denote a "partial" (often used in partial derivatives or particle physics notation), alongside a few other symbols frequently used in particle physics.

- 여기에는 '부분'을 나타내는 수학 기호(부분 미분이나 입자 물리학 표기에서 자주 사용됨)와 입자 물리학에서 자주 쓰이는 몇 가지 다른 기호들을 간단히 정리한 참고 자료가 있습니다.

# 주요 그리스 문자 유니코드

- Alpha (알파): 대문자 `U+0391` (`A`), 소문자 `U+03B1` (`α`)
- Beta (베타): 대문자 `U+0392` (`B`), 소문자 `U+03B2` (`β`)
- Gamma (감마): 대문자 `U+0393` (`Γ`), 소문자 `U+03B3` (`γ`)
- Delta (델타): 대문자 `U+0394` (`Δ`), 소문자 `U+03B4` (`δ`)
- Pi (파이/피): 대문자 `U+03A0` (`Π`), 소문자 `U+03C0` (`π`)
- Rho (로): 대문자 `U+03A1` (`Ρ`), 소문자 `U+03C1` (`ρ`)
- Phi (피/파이): 대문자 `U+03A6` (`Φ`), 소문자 `U+03C6` (`φ`)

- https://ko.wiktionary.org/wiki/%EB%B6%84%EB%A5%98:%EC%9C%A0%EB%8B%88%EC%BD%94%EB%93%9C_Greek_and_Coptic 


<hr />


## 1. General Navier–Stokes equation

For a Newtonian fluid, one common form is:

$$\rho\left(\frac{\partial \mathbf{u}}{\partial t}+(\mathbf{u}\cdot\nabla)\mathbf{u}\right)=-\nabla p+\mu\nabla^2\mathbf{u}+\mathbf{f}$$

### Meaning of the symbols

| Symbol         | uppercase/<br />lowercase   |Meaning                        |
| -------------- | -|----------------------------- |
| \(\rho\)       |`P`, `ρ`|Fluid density                  |
| \(\mathbf{u}\) |`u` | Fluid velocity vector          |
| \(t\)          | | Time                           |
| \(p\)          |`p`| Pressure                       |
| \(\mu\)        |`Μ`, `μ` | Dynamic viscosity              |
| \(\mathbf{f}\) |`f`| External force per unit volume |
| \(\nabla\)     |`∇` | Gradient operator              |
| \(\nabla^2\)   |`∇^2` |Laplacian operator             |


### How to Use `\mathbf`

- Basic syntax: `$\mathbf{A}$` produces upright bold A.
  - 기본 문법: `$\mathbf{A}$`는 세로로 된 굵은 A를 생성합니다. 

- Multiple characters: `$\mathbf{v} = \mathbf{a} + \mathbf{b}$`

$\mathbf{v} = \mathbf{a} + \mathbf{b}$

- https://trybibby.com/learn/bold-math-symbols 

---

## 2. Incompressible Navier–Stokes equations

For an **incompressible fluid** with constant density, the equations are usually written as:

$$\boxed{\frac{\partial \mathbf{u}}{\partial t}+(\mathbf{u}\cdot\nabla)\mathbf{u}=-\frac{1}{\rho}\nabla p+\nu\nabla^2\mathbf{u}+\mathbf{g}}$$

together with the **incompressibility condition**:

$$
\boxed{
\nabla\cdot\mathbf{u}=0
}
$$

Here,

$$
\nu = \frac{\mu}{\rho}
$$

is the **kinematic viscosity**.

---

## 3. What each term physically means

$$\underbrace{\frac{\partial \mathbf{u}}{\partial t}}_{\text{Change over time}}+\underbrace{(\mathbf{u}\cdot\nabla)\mathbf{u}}_{\text{Fluid carries itself}}=\underbrace{-\frac{1}{\rho}\nabla p}_{\text{Pressure force}}+\underbrace{\nu\nabla^2\mathbf{u}}_{\text{Viscosity}}+\underbrace{\mathbf{g}}_{\text{External forces}}$$

The famous nonlinear term is:

$$
(\mathbf{u}\cdot\nabla)\mathbf{u}
$$

This represents **convection (advection)**: fluid moving from one location to another carries its velocity with it. This nonlinear behavior is one reason turbulence and fluid motion can become extremely complicated.

---

## 4. Expanded 3D form

If the velocity is

$$
\mathbf{u}=(u,v,w)
$$

then the \(x\)-component, for example, is:

$$\frac{\partial u}{\partial t}+u\frac{\partial u}{\partial x}+v\frac{\partial u}{\partial y}+w\frac{\partial u}{\partial z}=
-\frac{1}{\rho}\frac{\partial p}{\partial x}
+
\nu
\left(
\frac{\partial^2u}{\partial x^2}
+
\frac{\partial^2u}{\partial y^2}
+
\frac{\partial^2u}{\partial z^2}
\right)
+
g_x
$$

There are similar equations for the \(y\)- and \(z\)-components.

---

### A simple way to remember it

> **Acceleration of fluid = pressure forces + viscous forces + external forces**

The Navier–Stokes equations are especially important in **aerodynamics, weather simulation, ocean modeling, computer graphics, CFD, and engineering**.

If you'd like, I can next explain the Navier–Stokes equation **line by line**, including exactly what operators such as ($$\nabla$$), ($$\nabla\cdot\$$), and ($$\nabla^2\$$) mean with simple examples.



# 나비에 -스토크스 방정식 유튜브 영상
- 유튜브 영상
  - [260822유체의 움직임을 예측하는 공식 | 10kg 나무는 뜨는데 1kg 돌은 왜 가라앉을까? 아르키메데스, 뉴턴, 나비에-스토크스까지! 우리가 몰랐던 유체의 모든 것 (feat. 민태기 박사)_취미는 과학/98화 확장판- EBS 컬렉션 - 사이언스](https://youtu.be/egfRLw2yzng?si=CKFoSAVZC-XM3ivM)
- 정의
  - [나무위키 NS Equation 정의 ](https://namu.wiki/w/%EB%82%98%EB%B9%84%EC%97%90-%EC%8A%A4%ED%86%A0%ED%81%AC%EC%8A%A4%20%EB%B0%A9%EC%A0%95%EC%8B%9D)
  - [한글_위키Wiki(NS Equation) 정의](https://ko.wikipedia.org/wiki/%EB%82%98%EB%B9%84%EC%97%90-%EC%8A%A4%ED%86%A0%ED%81%AC%EC%8A%A4_%EB%B0%A9%EC%A0%95%EC%8B%9D)
  - [영문Eng.)위키Wiki(NS Equation) 정의](https://en.wikipedia.org/wiki/Navier%E2%80%93Stokes_equations)
