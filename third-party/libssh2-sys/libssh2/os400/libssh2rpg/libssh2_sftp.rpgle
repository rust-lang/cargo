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

      /if not defined(LIBSSH2_SFTP_H_)
      /define LIBSSH2_SFTP_H_

      /include "libssh2rpg/libssh2"

      * Note: Version 6 was documented at the time of writing
      * However it was marked as "DO NOT IMPLEMENT" due to pending changes
      *
      * Let's start with Version 3 (The version found in OpenSSH) and go from
      * there.
     d LIBSSH2_SFTP_VERSION...
     d                 c                   3

      * Flags for open_ex().
     d LIBSSH2_SFTP_OPENFILE...
     d                 c                   0
     d LIBSSH2_SFTP_OPENDIR...
     d                 c                   1

      * Flags for rename_ex().
     d LIBSSH2_SFTP_RENAME_OVERWRITE...
     d                 c                   X'00000001'
     d LIBSSH2_SFTP_RENAME_ATOMIC...
     d                 c                   X'00000002'
     d LIBSSH2_SFTP_RENAME_NATIVE...
     d                 c                   X'00000004'

      * Flags for stat_ex().
     d LIBSSH2_SFTP_STAT...
     d                 c                   0
     d LIBSSH2_SFTP_LSTAT...
     d                 c                   1
     d LIBSSH2_SFTP_SETSTAT...
     d                 c                   2

      * Flags for symlink_ex().
     d LIBSSH2_SFTP_SYMLINK...
     d                 c                   0
     d LIBSSH2_SFTP_READLINK...
     d                 c                   1
     d LIBSSH2_SFTP_REALPATH...
     d                 c                   2

      * Flags for sftp_mkdir()
     d LIBSSH2_SFTP_DEFAULT_MODE...
     d                 c                   -1

      * SFTP attribute flag bits.
     d LIBSSH2_SFTP_ATTR_SIZE...
     d                 c                   X'00000001'
     d LIBSSH2_SFTP_ATTR_UIDGID...
     d                 c                   X'00000002'
     d LIBSSH2_SFTP_ATTR_PERMISSIONS...
     d                 c                   X'00000004'
     d LIBSSH2_SFTP_ATTR_ACMODTIME...
     d                 c                   X'00000008'
     d LIBSSH2_SFTP_ATTR_EXTENDED...
     d                 c                   X'80000000'

      * SFTP statvfs flag bits.
     d LIBSSH2_SFTP_ST_RDONLY...
     d                 c                   X'00000001'
     d LIBSSH2_SFTP_ST_NOSUID...
     d                 c                   X'00000002'

     d LIBSSH2_SFTP_ATTRIBUTES...
     d                 ds                  based(######typedef######)
     d                                     align qualified
      * If flags & ATTR_* bit is set, then the value in this struct will be
      * meaningful Otherwise it should be ignored.
     d  flags                              like(libssh2_Culong)
     d  filesize                           like(libssh2_uint64_t)
     d  uid                                like(libssh2_Culong)
     d  gid                                like(libssh2_Culong)
     d  permissions                        like(libssh2_Culong)
     d  atime                              like(libssh2_Culong)
     d  mtime                              like(libssh2_Culong)

     d #LIBSSH2_SFTP_STATVFS...
     d                 ds                  based(######typedef######)
     d                                     align qualified
     d  f_bsize                            like(libssh2_uint64_t)               Filesys block size
     d  f_frsize                           like(libssh2_uint64_t)               Fragment size
     d  f_blocks                           like(libssh2_uint64_t)               FS size in f_frsize
     d  f_bfree                            like(libssh2_uint64_t)               Free blocks
     d  f_bavail                           like(libssh2_uint64_t)               Free blks f. nonroot
     d  f_files                            like(libssh2_uint64_t)               Inodes
     d  f_ffree                            like(libssh2_uint64_t)               Free inodes
     d  f_favail                           like(libssh2_uint64_t)               Free inds f. nonroot
     d  f_fsid                             like(libssh2_uint64_t)               File system ID
     d  f_flag                             like(libssh2_uint64_t)               Mount flags
     d  f_namemax                          like(libssh2_uint64_t)               Max filename length

      * SFTP filetypes.
     d LIBSSH2_SFTP_TYPE_REGULAR...
     d                 c                   1
     d LIBSSH2_SFTP_TYPE_DIRECTORY...
     d                 c                   2
     d LIBSSH2_SFTP_TYPE_SYMLINK...
     d                 c                   3
     d LIBSSH2_SFTP_TYPE_SPECIAL...
     d                 c                   4
     d LIBSSH2_SFTP_TYPE_UNKNOWN...
     d                 c                   5
     d LIBSSH2_SFTP_TYPE_SOCKET...
     d                 c                   6
     d LIBSSH2_SFTP_TYPE_CHAR_DEVICE...
     d                 c                   7
     d LIBSSH2_SFTP_TYPE_BLOCK_DEVICE...
     d                 c                   8
     d LIBSSH2_SFTP_TYPE_FIFO...
     d                 c                   9

      * Reproduce the POSIX file modes here for systems that are not POSIX
      * compliant.
      *
      * These is used in "permissions" of "struct _LIBSSH2_SFTP_ATTRIBUTES"

      * File type.
     d LIBSSH2_SFTP_S_IFMT...                                                   type of file mask
     d                 c                   X'F000'
     d LIBSSH2_SFTP_S_IFIFO...                                                  named pipe (fifo)
     d                 c                   X'1000'
     d LIBSSH2_SFTP_S_IFCHR...                                                  character special
     d                 c                   X'2000'
     d LIBSSH2_SFTP_S_IFDIR...                                                  directory
     d                 c                   X'4000'
     d LIBSSH2_SFTP_S_IFBLK...                                                  block special
     d                 c                   X'6000'
     d LIBSSH2_SFTP_S_IFREG...                                                  regular
     d                 c                   X'8000'
     d LIBSSH2_SFTP_S_IFLNK...                                                  symbolic link
     d                 c                   X'A000'
     d LIBSSH2_SFTP_S_IFSOCK...                                                 socket
     d                 c                   X'C000'

      * File mode.
      * Read, write, execute/search by owner.
     d LIBSSH2_SFTP_S_IRWXU...                                                  RWX mask for owner
     d                 c                   X'01C0'
     d LIBSSH2_SFTP_S_IRUSR...                                                  R for owner
     d                 c                   X'0100'
     d LIBSSH2_SFTP_S_IWUSR...                                                  W for owner
     d                 c                   X'0080'
     d LIBSSH2_SFTP_S_IXUSR...                                                  X for owner
     d                 c                   X'0040'
      * Read, write, execute/search by group.
     d LIBSSH2_SFTP_S_IRWXG...                                                  RWX mask for group
     d                 c                   X'0038'
     d LIBSSH2_SFTP_S_IRGRP...                                                  R for group
     d                 c                   X'0020'
     d LIBSSH2_SFTP_S_IWGRP...                                                  W for group
     d                 c                   X'0010'
     d LIBSSH2_SFTP_S_IXGRP...                                                  X for group
     d                 c                   X'0008'
      * Read, write, execute/search by others.
     d LIBSSH2_SFTP_S_IRWXO...                                                  RWX mask for other
     d                 c                   X'0007'
     d LIBSSH2_SFTP_S_IROTH...                                                  R for other
     d                 c                   X'0004'
     d LIBSSH2_SFTP_S_IWOTH...                                                  W for other
     d                 c                   X'0002'
     d LIBSSH2_SFTP_S_IXOTH...                                                  X for other
     d                 c                   X'0001'

      * C macro implementation.
     d LIBSSH2_SFTP_S_ISLNK...
     d                 pr                  extproc('LIBSSH2_SFTP_S_ISLNK')
     d                                     like(libssh2_Cint)
     d  permissions                        value like(libssh2_Culong)

      * C macro implementation.
     d LIBSSH2_SFTP_S_ISREG...
     d                 pr                  extproc('LIBSSH2_SFTP_S_ISREG')
     d                                     like(libssh2_Cint)
     d  permissions                        value like(libssh2_Culong)

      * C macro implementation.
     d LIBSSH2_SFTP_S_ISDIR...
     d                 pr                  extproc('LIBSSH2_SFTP_S_ISDIR')
     d                                     like(libssh2_Cint)
     d  permissions                        value like(libssh2_Culong)

      * C macro implementation.
     d LIBSSH2_SFTP_S_ISCHR...
     d                 pr                  extproc('LIBSSH2_SFTP_S_ISCHR')
     d                                     like(libssh2_Cint)
     d  permissions                        value like(libssh2_Culong)

      * C macro implementation.
     d LIBSSH2_SFTP_S_ISBLK...
     d                 pr                  extproc('LIBSSH2_SFTP_S_ISBLK')
     d                                     like(libssh2_Cint)
     d  permissions                        value like(libssh2_Culong)

      * C macro implementation.
     d LIBSSH2_SFTP_S_ISFIFO...
     d                 pr                  extproc('LIBSSH2_SFTP_S_ISFIFO')
     d                                     like(libssh2_Cint)
     d  permissions                        value like(libssh2_Culong)

      * C macro implementation.
     d LIBSSH2_SFTP_S_ISSOCK...
     d                 pr                  extproc('LIBSSH2_SFTP_S_ISSOCK')
     d                                     like(libssh2_Cint)
     d  permissions                        value like(libssh2_Culong)

      * SFTP File Transfer Flags -- (e.g. flags parameter to sftp_open())
      * Danger will robinson... APPEND doesn't have any effect on OpenSSH
      * servers.
     d LIBSSH2_FXF_READ...
     d                 c                   X'00000001'
     d LIBSSH2_FXF_WRITE...
     d                 c                   X'00000002'
     d LIBSSH2_FXF_APPEND...
     d                 c                   X'00000004'
     d LIBSSH2_FXF_CREAT...
     d                 c                   X'00000008'
     d LIBSSH2_FXF_TRUNC...
     d                 c                   X'00000010'
     d LIBSSH2_FXF_EXCL...
     d                 c                   X'00000020'

      * SFTP Status Codes (returned by libssh2_sftp_last_error()).
     d LIBSSH2_FX_OK...
     d                 c                   0
     d LIBSSH2_FX_EOF...
     d                 c                   1
     d LIBSSH2_FX_NO_SUCH_FILE...
     d                 c                   2
     d LIBSSH2_FX_PERMISSION_DENIED...
     d                 c                   3
     d LIBSSH2_FX_FAILURE...
     d                 c                   4
     d LIBSSH2_FX_BAD_MESSAGE...
     d                 c                   5
     d LIBSSH2_FX_NO_CONNECTION...
     d                 c                   6
     d LIBSSH2_FX_CONNECTION_LOST...
     d                 c                   7
     d LIBSSH2_FX_OP_UNSUPPORTED...
     d                 c                   8
     d LIBSSH2_FX_INVALID_HANDLE...
     d                 c                   9
     d LIBSSH2_FX_NO_SUCH_PATH...
     d                 c                   10
     d LIBSSH2_FX_FILE_ALREADY_EXISTS...
     d                 c                   11
     d LIBSSH2_FX_WRITE_PROTECT...
     d                 c                   12
     d LIBSSH2_FX_NO_MEDIA...
     d                 c                   13
     d LIBSSH2_FX_NO_SPACE_ON_FILESYSTEM...
     d                 c                   14
     d LIBSSH2_FX_QUOTA_EXCEEDED...
     d                 c                   15
     d LIBSSH2_FX_UNKNOWN_PRINCIPAL...
     d                 c                   16
     d LIBSSH2_FX_LOCK_CONFLICT...
     d                 c                   17
     d LIBSSH2_FX_DIR_NOT_EMPTY...
     d                 c                   18
     d LIBSSH2_FX_NOT_A_DIRECTORY...
     d                 c                   19
     d LIBSSH2_FX_INVALID_FILENAME...
     d                 c                   20
     d LIBSSH2_FX_LINK_LOOP...
     d                 c                   21

      * Returned by any function that would block during a read/write operation.
     d LIBSSH2SFTP_EAGAIN...
     d                 c                   -37

      * SFTP API.
     d libssh2_sftp_init...
     d                 pr              *   extproc('libssh2_sftp_init')         LIBSSH2_SFTP *
     d  session                        *   value                                LIBSSH2_SESSION *

     d libssh2_sftp_shutdown...
     d                 pr                  extproc('libssh2_sftp_shutdown')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *

     d libssh2_sftp_last_error...
     d                 pr                  extproc('libssh2_sftp_last_error')
     d                                     like(libssh2_Culong)
     d  sftp                           *   value                                LIBSSH2_SFTP *

     d libssh2_sftp_get_channel...
     d                 pr              *   extproc('libssh2_sftp_get_channel')  LIBSSH2_CHANNEL *
     d  sftp                           *   value                                LIBSSH2_SFTP *

      * File / Directory Ops.
     d libssh2_sftp_open_ex...
     d                 pr              *   extproc('libssh2_sftp_open_ex')      LIBSSH2_SFTP_HANDLE*
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  filename                       *   value options(*string)               const char *
     d  filename_len                       value like(libssh2_Cuint)
     d  flags                              value like(libssh2_Culong)
     d  mode                               value like(libssh2_Clong)
     d  open_type                          value like(libssh2_Cint)

      * C macro implementation.
     d libssh2_sftp_open...
     d                 pr              *   extproc('libssh2_sftp_open')         LIBSSH2_SFTP_HANDLE*
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  filename                       *   value options(*string)               const char *
     d  flags                              value like(libssh2_Culong)
     d  mode                               value like(libssh2_Clong)

      * C macro libssh2_sftp_opendir implementation.
      * Renamed to avoid upper/lower case name clash.
     d libssh2_sftp_open_dir...
     d                 pr              *   extproc('libssh2_sftp_opendir')      LIBSSH2_SFTP_HANDLE*
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  path                           *   value options(*string)               const char *

     d libssh2_sftp_open_ex_r...
     d                 pr              *   extproc('libssh2_sftp_open_ex_r')    LIBSSH2_SFTP_HANDLE*
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  filename                       *   value options(*string)               const char *
     d  filename_len                       value like(libssh2_Csize_t)
     d  flags                              value like(libssh2_Culong)
     d  mode                               value like(libssh2_Clong)
     d  open_type                          value like(libssh2_Cint)
     d  attrs                              likeds(LIBSSH2_SFTP_ATTRIBUTES)

      * C macro implementation.
     d libssh2_sftp_open_r...
     d                 pr              *   extproc('libssh2_sftp_open_r')       LIBSSH2_SFTP_HANDLE*
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  filename                       *   value options(*string)               const char *
     d  flags                              value like(libssh2_Culong)
     d  mode                               value like(libssh2_Clong)
     d  attrs                              likeds(LIBSSH2_SFTP_ATTRIBUTES)

     d libssh2_sftp_read...
     d                 pr                  extproc('libssh2_sftp_read')
     d                                     like(libssh2_Cssize_t)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*
     d  buffer                         *   value                                char *
     d  buffer_maxlen                      value like(libssh2_Csize_t)

     d libssh2_sftp_readdir_ex...
     d                 pr                  extproc('libssh2_sftp_readdir_ex')
     d                                     like(libssh2_Cint)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*
     d  buffer                         *   value                                char *
     d  buffer_maxlen                      value like(libssh2_Csize_t)
     d  longentry                      *   value                                char *
     d  longentry_maxlen...
     d                                     value like(libssh2_Csize_t)
     d  attrs                          *   value                                LIBSSH2_SFTP_...
     d                                                                          ATTRIBUTES *

      * C macro implementation.
     d libssh2_sftp_readdir...
     d                 pr                  extproc('libssh2_sftp_readdir')
     d                                     like(libssh2_Cint)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*
     d  buffer                         *   value                                char *
     d  buffer_maxlen                      value like(libssh2_Csize_t)
     d  attrs                          *   value                                LIBSSH2_SFTP_...
     d                                                                          ATTRIBUTES *

     d libssh2_sftp_write...
     d                 pr                  extproc('libssh2_sftp_write')
     d                                     like(libssh2_Cssize_t)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*
     d  buffer                         *   value options(*string)               const char *
     d  count                              value like(libssh2_Csize_t)

     d libssh2_sftp_fsync...
     d                 pr                  extproc('libssh2_sftp_fsync')
     d                                     like(libssh2_Cint)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*

     d libssh2_sftp_close_handle...
     d                 pr                  extproc('libssh2_sftp_close_handle')
     d                                     like(libssh2_Cint)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*

      * C macro implementation.
     d libssh2_sftp_close...
     d                 pr                  extproc('libssh2_sftp_close_handle')
     d                                     like(libssh2_Cint)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*

      * C macro implementation.
     d libssh2_sftp_closedir...
     d                 pr                  extproc('libssh2_sftp_close_handle')
     d                                     like(libssh2_Cint)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*

     d libssh2_sftp_seek...
     d                 pr                  extproc('libssh2_sftp_seek')
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*
     d  offset                             value like(libssh2_Csize_t)

     d libssh2_sftp_seek64...
     d                 pr                  extproc('libssh2_sftp_seek64')
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*
     d  offset                             value like(libssh2_uint64_t)

      * C macro implementation.
     d libssh2_sftp_rewind...
     d                 pr                  extproc('libssh2_sftp_rewind')
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*

     d libssh2_sftp_tell...
     d                 pr                  extproc('libssh2_sftp_tell')
     d                                     like(libssh2_Csize_t)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*

     d libssh2_sftp_tell64...
     d                 pr                  extproc('libssh2_sftp_tell64')
     d                                     like(libssh2_uint64_t)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*

     d libssh2_sftp_fstat_ex...
     d                 pr                  extproc('libssh2_sftp_fstat_ex')
     d                                     like(libssh2_Cint)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*
     d  attrs                          *   value                                LIBSSH2_SFTP_...
     d                                                                          ATTRIBUTES *
     d  setstat                            value like(libssh2_Cint)

      * C macro implementation.
     d libssh2_sftp_fstat...
     d                 pr                  extproc('libssh2_sftp_fstat')
     d                                     like(libssh2_Cint)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*
     d  attrs                          *   value                                LIBSSH2_SFTP_...
     d                                                                          ATTRIBUTES *

      * C macro implementation.
     d libssh2_sftp_fsetstat...
     d                 pr                  extproc('libssh2_sftp_fsetstat')
     d                                     like(libssh2_Cint)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*
     d  attrs                          *   value                                LIBSSH2_SFTP_...
     d                                                                          ATTRIBUTES *

      * Miscellaneous Ops.
     d libssh2_sftp_rename_ex...
     d                 pr                  extproc('libssh2_sftp_rename_ex')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  source_filename...
     d                                 *   value options(*string)               const char *
     d  source_filename_len...
     d                                     value like(libssh2_Cuint)
     d  dest_filename                  *   value options(*string)               const char *
     d  dest_filename_len...
     d                                     value like(libssh2_Cuint)
     d  flags                              value like(libssh2_Clong)

      * C macro implementation.
     d libssh2_sftp_rename...
     d                 pr                  extproc('libssh2_sftp_rename')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  source_filename...
     d                                 *   value options(*string)               const char *
     d  dest_filename                  *   value options(*string)               const char *

     d libssh2_sftp_unlink_ex...
     d                 pr                  extproc('libssh2_sftp_unlink_ex')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  filename                       *   value options(*string)               const char *
     d  filename_len                       value like(libssh2_Cuint)

      * C macro implementation.
     d libssh2_sftp_unlink...
     d                 pr                  extproc('libssh2_sftp_unlink')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  filename                       *   value options(*string)               const char *

     d libssh2_sftp_fstatvfs...
     d                 pr                  extproc('libssh2_sftp_fstatvfs')
     d                                     like(libssh2_Cint)
     d  handle                         *   value                                LIBSSH2_SFTP_HANDLE*
     d  st                             *   value                                LIBSSH2_SFTP_STATVFS
     d                                                                          *

     d libssh2_sftp_statvfs...
     d                 pr                  extproc('libssh2_sftp_statvfs')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  path                           *   value options(*string)               const char *
     d  path_len                           value like(libssh2_Csize_t)
     d  st                             *   value                                LIBSSH2_SFTP_STATVFS
     d                                                                          *

     d libssh2_sftp_mkdir_ex...
     d                 pr                  extproc('libssh2_sftp_mkdir_ex')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  path                           *   value options(*string)               const char *
     d  path_len                           value like(libssh2_Cuint)
     d  mode                               value like(libssh2_Clong)

      * C macro implementation.
     d libssh2_sftp_mkdir...
     d                 pr                  extproc('libssh2_sftp_mkdir')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  path                           *   value options(*string)               const char *
     d  mode                               value like(libssh2_Clong)

     d libssh2_sftp_rmdir_ex...
     d                 pr                  extproc('libssh2_sftp_rmdir_ex')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  path                           *   value options(*string)               const char *
     d  path_len                           value like(libssh2_Cuint)

      * C macro implementation.
     d libssh2_sftp_rmdir...
     d                 pr                  extproc('libssh2_sftp_rmdir')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  path                           *   value options(*string)               const char *

     d libssh2_sftp_stat_ex...
     d                 pr                  extproc('libssh2_sftp_stat_ex')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  path                           *   value options(*string)               const char *
     d  path_len                           value like(libssh2_Cuint)
     d  stat_type                          value like(libssh2_Cint)
     d  attrs                          *   value                                LIBSSH2_SFTP_...
     d                                                                          ATTRIBUTES *

      * C macro libssh2_sftp_stat implementation.
      * Renamed to avoid upper/lower case name clash.
     d libssh2_sftp_get_stat...
     d                 pr                  extproc('libssh2_sftp_stat')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  path                           *   value options(*string)               const char *
     d  attrs                          *   value                                LIBSSH2_SFTP_...
     d                                                                          ATTRIBUTES *

      * C macro libssh2_sftp_lstat implementation.
      * Renamed to avoid upper/lower case name clash.
     d libssh2_sftp_get_lstat...
     d                 pr                  extproc('libssh2_sftp_lstat')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  path                           *   value options(*string)               const char *
     d  attrs                          *   value                                LIBSSH2_SFTP_...
     d                                                                          ATTRIBUTES *

      * C macro libssh2_sftp_setstat implementation.
      * Renamed to avoid upper/lower case name clash.
     d libssh2_sftp_set_stat...
     d                 pr                  extproc('libssh2_sftp_setstat')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  path                           *   value options(*string)               const char *
     d  attrs                          *   value                                LIBSSH2_SFTP_...
     d                                                                          ATTRIBUTES *

     d libssh2_sftp_symlink_ex...
     d                 pr                  extproc('libssh2_sftp_symlink_ex')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  path                           *   value options(*string)               const char *
     d  path_len                           value like(libssh2_Cuint)
     d  target                         *   value options(*string)               char *
     d  target_len                         value like(libssh2_Cuint)
     d  link_type                          value like(libssh2_Cint)

      * C macro libssh2_sftp_symlink implementation.
      * Renamed to avoid upper/lower case name clash.
     d libssh2_sftp_sym_link...
     d                 pr                  extproc('libssh2_sftp_symlink')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  orig                           *   value options(*string)               const char *
     d  linkpath                       *   value options(*string)               char *

      * C macro libssh2_sftp_readlink implementation.
      * Renamed to avoid upper/lower case name clash.
     d libssh2_sftp_read_link...
     d                 pr                  extproc('libssh2_sftp_readlink')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  path                           *   value options(*string)               const char *
     d  target                         *   value                                char *
     d  maxlen                             value like(libssh2_Cuint)

      * C macro libssh2_sftp_realpath implementation.
      * Renamed to avoid upper/lower case name clash.
     d libssh2_sftp_real_path...
     d                 pr                  extproc('libssh2_sftp_realpath')
     d                                     like(libssh2_Cint)
     d  sftp                           *   value                                LIBSSH2_SFTP *
     d  path                           *   value options(*string)               const char *
     d  target                         *   value                                char *
     d  maxlen                             value like(libssh2_Cuint)

      /endif                                                                    LIBSSH2_SFTP_H_
