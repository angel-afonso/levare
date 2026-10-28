Levare para Linux
=================

Abrir Levare: ejecuta ./levare desde esta carpeta.

Añadirlo al menú de aplicaciones: ejecuta ./install.sh. Copia el icono y una
entrada de menú en ~/.local/share; Levare sigue en esta carpeta.

Levare se actualiza solo dentro de esta carpeta, así que déjala donde puedas
escribir (por ejemplo ~/Aplicaciones/levare), no en /usr ni en /opt.

Necesita:
- una distribución con glibc 2.35 o posterior: Ubuntu 22.04, Debian 12,
  Fedora 36 o más recientes;
- ALSA (libasound2), con PulseAudio o PipeWire a través de su complemento ALSA;
- D-Bus (libdbus-1-3), para dar prioridad al sonido;
- Vulkan (el controlador Mesa o el de tu tarjeta gráfica);
- xdg-desktop-portal, para los diálogos de abrir y guardar.

libsfizz.so es el motor de sonidos SFZ (sfizz 1.2.3, licencia BSD-2-Clause);
sus avisos de licencia están en licenses/sfizz.
