# Navier-Stokes_equations_Vulkan
나비에-스토크스 방정식(Navier-Stokes equations, NS equation)


# 공식(NS Equation)(ChatGPT)

The **Navier–Stokes (NS) equations** describe the motion of fluids such as water, air, and gases. They express **Newton's second law applied to a fluid**.

# 유튜브에 나온 공식들.

- [공식 9분16초에 나온다.](https://youtu.be/egfRLw2yzng?si=rO9AS-sfR9Aue680)


$$
\rho\left(
    \frac{\partial u_i}{\partial t}
    +
    u_j \frac{\partial u_i}{\partial x_j}
\right)
=
-\frac{\partial p}{\partial x_i}
+
\mu\frac{\partial^2 u_i}{\partial x_j \partial x_j} 
$$

- P = 압력
  - 압력(P) 이 유체의 속도(u) 를 좌우한다.

$$
 \partial u_i
 \partial u_j
$$ 

- 속도(속도를 나타내는 표현들)



<hr />


## 1. General Navier–Stokes equation

For a Newtonian fluid, one common form is:

$$\rho\left(\frac{\partial \mathbf{u}}{\partial t}+(\mathbf{u}\cdot\nabla)\mathbf{u}\right)=-\nabla p+\mu\nabla^2\mathbf{u}+\mathbf{f}$$

### Meaning of the symbols

| Symbol         | Meaning                        |
| -------------- | ------------------------------ |
| \(\rho\)       | Fluid density                  |
| \(\mathbf{u}\) | Fluid velocity vector          |
| \(t\)          | Time                           |
| \(p\)          | Pressure                       |
| \(\mu\)        | Dynamic viscosity              |
| \(\mathbf{f}\) | External force per unit volume |
| \(\nabla\)     | Gradient operator              |
| \(\nabla^2\)   | Laplacian operator             |

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
