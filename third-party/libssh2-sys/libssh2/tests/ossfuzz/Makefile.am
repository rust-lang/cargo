AM_CPPFLAGS = -I$(top_builddir)/include
LDADD = $(top_builddir)/src/libssh2.la

if USE_OSSFUZZ_FLAG
FUZZ_FLAG = $(LIB_FUZZING_ENGINE)
else
if USE_OSSFUZZ_STATIC
LDADD += $(LIB_FUZZING_ENGINE)
FUZZ_FLAG =
else
LDADD += libstandaloneengine.a
FUZZ_FLAG =
endif
endif

noinst_PROGRAMS =
noinst_LIBRARIES =

if USE_OSSFUZZERS
noinst_PROGRAMS += \
  ssh2_client_fuzzer

noinst_LIBRARIES += \
  libstandaloneengine.a
endif

ssh2_client_fuzzer_SOURCES = ssh2_client_fuzzer.cc testinput.h
ssh2_client_fuzzer_CXXFLAGS = $(AM_CXXFLAGS) $(FUZZ_FLAG)
ssh2_client_fuzzer_LDFLAGS = $(AM_LDFLAGS) -static

libstandaloneengine_a_SOURCES = standaloneengine.cc
libstandaloneengine_a_CXXFLAGS = $(AM_CXXFLAGS)

EXTRA_DIST = ossfuzz.sh
