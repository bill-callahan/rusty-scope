# Modeling the Sky and the Mount

## Overview
The pointing model and coordinate systems are fundamental to the operation of the telescope mount. This section covers the mathematical models used to translate celestial coordinates to mount coordinates and vice versa.

## Celestial Coordinate Systems
### Equatorial Coordinates
- Right Ascension (RA): The angular distance of a point east of the vernal equinox, measured along the celestial equator.
- Declination (Dec): The angular distance of a point north or south of the celestial equator.

### Horizontal Coordinates
- Altitude (Alt): The angular distance of a point above the observer's horizon.
- Azimuth (Az): The angular distance of a point east of north, measured along the observer's horizon.

## Mount Coordinate Systems
### Alt-Azimuth Mount
- Altitude: The angular distance of the telescope's optical axis above the observer's horizon.
- Azimuth: The angular distance of the telescope's optical axis east of north, measured along the observer's horizon.

### Equatorial Mount
- Hour Angle (HA): The angular distance of a point west of the observer's meridian, measured along the celestial equator.
- Declination (Dec): The angular distance of a point north or south of the celestial equator.

## Coordinate Transformations
### Equatorial to Horizontal
The transformation from equatorial coordinates to horizontal coordinates involves converting right ascension and declination to altitude and azimuth. This transformation is necessary for pointing the telescope at a specific celestial object.

### Horizontal to Equatorial
The transformation from horizontal coordinates to equatorial coordinates involves converting altitude and azimuth to right ascension and declination. This transformation is necessary for tracking celestial objects as they move across the sky.

### Equatorial to Mount Coordinates
The transformation from equatorial coordinates to mount coordinates involves converting right ascension and declination to altitude and azimuth for an alt-azimuth mount, or to hour angle and declination for an equatorial mount. This transformation is necessary for controlling the telescope's movement.

### Mount Coordinates to Equatorial
The transformation from mount coordinates to equatorial coordinates involves converting altitude and azimuth to right ascension and declination for an alt-azimuth mount, or to right ascension and declination for an equatorial mount. This transformation is necessary for determining the telescope's pointing direction.

## Pointing Model
### Basic Pointing Model
The basic pointing model is a simplified model that assumes the telescope is perfectly aligned with the celestial coordinate system. This model is suitable for basic pointing and tracking of celestial objects.

### Advanced Pointing Model with Refraction Correction
The advanced pointing model with refraction correction accounts for the atmospheric refraction of light, which causes celestial objects to appear higher in the sky than they actually are. This model is necessary for accurate pointing and tracking of celestial objects.