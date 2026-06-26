/*
 * Copyright (C) Patrick Monnerat <patrick@monnerat.net>
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms,
 * with or without modification, are permitted provided
 * that the following conditions are met:
 *
 *   Redistributions of source code must retain the above
 *   copyright notice, this list of conditions and the
 *   following disclaimer.
 *
 *   Redistributions in binary form must reproduce the above
 *   copyright notice, this list of conditions and the following
 *   disclaimer in the documentation and/or other materials
 *   provided with the distribution.
 *
 *   Neither the name of the copyright holder nor the names
 *   of any other contributors may be used to endorse or
 *   promote products derived from this software without
 *   specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND
 * CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
 * INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES
 * OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT OWNER OR
 * CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
 * SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING,
 * BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
 * WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
 * NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
 * USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY
 * OF SUCH DAMAGE.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#ifndef LIBSSH2_SYS_SOCKET_H
#define LIBSSH2_SYS_SOCKET_H

/*
 *  <sys/socket.h> wrapper.
 *  Redefines connect().
 */

#include <qadrt.h>

#ifndef _QADRT_LT
# define _QADRT_LT <
#endif
#ifndef _QADRT_GT
# define _QADRT_GT >
#endif

#ifdef QADRT_SYSINC
# include _QADRT_LT QADRT_SYSINC/sys/socket.h _QADRT_GT
#elif __ILEC400_TGTVRM__ >= 710
# include_next <sys/socket.h>
#elif !defined(__SRCSTMF__)
# include <QSYSINC/sys/socket>
#else
# include </QIBM/include/sys/socket.h>
#endif

extern int  _libssh2_os400_connect(int sd,
                                   struct sockaddr *destaddr, int addrlen);

#ifndef LIBSSH2_DISABLE_QADRT_EXT
#define connect(sd, addr, len)  _libssh2_os400_connect((sd), (addr), (len))
#endif

#endif

/* vim: set expandtab ts=4 sw=4: */
