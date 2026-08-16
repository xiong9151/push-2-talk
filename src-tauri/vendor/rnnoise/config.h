/* RNNoise configuration.
 *
 * NOTE: _USE_MATH_DEFINES is required on MSVC so that <math.h> exposes M_PI,
 * which denoise.c uses.  Without it, MSVC silently hides M_PI from the
 * standard library.
 */
#define OPUS_BUILD
#define USE_ALLOCA
#define _USE_MATH_DEFINES
#ifdef _WIN32
#include <malloc.h>
#endif