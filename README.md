# GridGame

Juego de estrategia en grilla hexagonal hecho con Rust y Macroquad. Permite construir edificios, explotar recursos y manejar unidades de logistica en rutas.

**Requisitos**
1. Rust y Cargo instalados.

**Ejecutar**
1. `cargo run`

**Controles**
1. Click izquierdo: abrir ventana de un bloque.
2. Click derecho: colocar bloque o iniciar deconstruccion (si esta seleccionado "Deconstruir").
3. Rueda del mouse: zoom.
4. Boton medio (arrastrar): mover camara.
5. `R`: rotar colocacion.
6. `Esc`: volver al menu (guarda la partida).

**Notas**
1. Las minas pueden expandir sus zonas anexadas desde la ventana del bloque.
2. Las unidades de logistica se crean desde la ventana del bloque Logistica y pueden listarse con el boton "Lista unidades".
