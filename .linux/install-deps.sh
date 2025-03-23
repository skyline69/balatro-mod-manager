#!/usr/bin/env sh
set -xe

apt-get update && apt-get install -y \
  curl \
  unzip \
  \
  pkg-config \
  file \
  xdg-utils \
  \
  libglib2.0-dev \
  libgtk-3-dev \
  libjavascriptcoregtk-4.1-dev \
  librsvg2-dev \
  libsoup-3.0-dev \
  libssl-dev \
  libwebkit2gtk-4.1-dev
