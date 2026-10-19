/**
 * The Spanish account page messages, written by hand.
 *
 * It is typed `AccountMessages`, so a key added to English and not here is a
 * `tsc` error.
 */
import type { AccountMessages } from '../index';

export const es: AccountMessages = {
  'password.heading': 'Establecer una contraseña',
  'password.new': 'Contraseña nueva',
  'password.confirm': 'Confirmar la contraseña nueva',
  'password.submit': 'Establecer la contraseña',
  'password.errorEmpty': 'Introduce una contraseña nueva.',
  'password.errorMismatch': 'Las dos contraseñas no coinciden.',
  'password.done': 'Tu contraseña está establecida. Inicia sesión desde el encabezado.',
  'password.missingKey':
    'Este enlace no lleva ninguna clave de contraseña. Solicita una nueva.',
  'forgot.heading': 'Restablecer tu contraseña',
  'forgot.submit': 'Enviar el enlace',
  'forgot.errorEmpty': 'Introduce tu dirección de correo electrónico.',
  'forgot.sent': 'Consulta tu correo para ver el enlace de restablecimiento.',
  'forgot.hint':
    'La respuesta es la misma coincida o no la dirección con una cuenta, así que esta página nunca dice qué direcciones existen.',
};
