# carbon

El updater de discada: echarle carbón nuevo al asador. A la izquierda los pasos
(con los sub-pasos de `install.sh`) y el log del de ahora; a la derecha la
discada en la lumbre, meneándose mientras se actualiza, el vaquero bailando
cuando queda lista y la lumbre apagada si algo falló. Lo que se pregunta (la
contraseña de sudo, tu huella, un `[S/n]`) se contesta ahí mismo. Con los
colores y el estilo del look (disco).

```
carbon                   actualizar discada
carbon prueba            la pantalla con pasos de mentira (no toca nada)
carbon revisa REPO BASE  qué archivos tuyos toca la actualización
                         (lo usa install.sh --update; era UpdateCheck.py)
```

Hace lo que hacía `hypr/scripts/Update.sh` (que ahora le pasa la estafeta):
si `~/.config` es el repo, guarda el look, `git pull --rebase --autostash`
(y se para sin recargar si chocó), vuelve a poner el look y recarga; si discada
se instaló con `install.sh`, `git pull` en `~/dotfiles` e `install.sh --update`.
Cada comando corre en una pty, así sus preguntas siguen funcionando. Sin
terminal, imprime los renglones y lo que llegue por la entrada contesta las
preguntas.

MIT.
