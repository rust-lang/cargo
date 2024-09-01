type string_program_path = string
type string_path = string
type string_url = string

/**
 * for completion, union over wide type `string`
 */
type target_arch = 'x86_64' | 'i686' | 'arm' | 'thumb' | 'mips'
type target_sub = 'v7' | 'v7s' | 'v5te'
type target_vendor = 'unknown' | 'apple' | 'pc' | 'nvidia'
type target_sys = 'linux' | 'windows' | 'darwin' | 'none'
type target_abi = 'gnu' | 'android' | 'eabi'
/**
 * @todo should only partial matix
 */
type string_target = `${target_arch}${target_sub}-${target_vendor}-${target_sys}-${target_abi}`

/**
 * @todo
 */
type string_rust_flag = string
type string_rustdoc_flag = string

type proxy_protocol = 'https'
type proxy_host = string
type proxy_port = `${number}`
type string_libcurl_proxy = `${proxy_protocol | ''}${proxy_host}${proxy_port | ''}`
type string_tls_version = 'default' | 'tlsv1' | 'tlsv1.0' | 'tlsv1.1' | 'tlsv1.2' | 'tlsv1.3'

/**
 * @todo
 */
type string_patch_name = string
type string_profile_name = string
type string_package_name = string
type string_registry_name = string
type string_creditial_alias_name = string

/**
 * @todo
 */
type string_triple = string
type string_cfg = `cfg(${string})`

/**
 * https://doc.rust-lang.org/cargo/reference/config.html
 */
interface CargoConfigSchema {
  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#paths
   *
   * @default undefined
   */
  paths?: string[]

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#alias
   *
   * @default bcdtrrm
   */
  alias?: { [key: string]: string | string[] }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#build
   */
  build?: {
    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#buildjobs
     *
     * @default number_of_logical_cpus
     */
    jobs?: number | string

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#buildrustc
     *
     * @default 'rustc'
     */
    rustc?: string_program_path

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#buildrustc-wrapper
     *
     * @default undefined
     */
    'rustc-wrapper'?: string_program_path

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#buildrustc-workspace-wrapper
     *
     * @default undefined
     */
    'rustc-workspace-wrapper'?: string_program_path

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#buildrustdoc
     *
     * @default 'rustdoc'
     */
    rustdoc?: string_program_path

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#buildtarget
     *
     * @default host_platform
     */
    target?: string_target | string_target[]

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#buildtarget-dir
     *
     * @default 'target'
     */
    'target-dir'?: string_program_path

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#buildrustflags
     *
     * @default undefined
     */
    rustflags?: string_rust_flag | string_rust_flag[]

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#buildrustdocflags
     *
     * @default undefined
     */
    rustdocflags?: string_rustdoc_flag

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#buildincremental
     *
     * @default from_profile
     */
    incremental?: boolean

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#builddep-info-basedir
     *
     * @default undefined
     */
    'dep-info-basedir'?: string_program_path

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#buildpipelining
     *
     * @deprecated
     */
    pipelinging?: never
  }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#credential-alias
   *
   * @todo not sure, just roughly impl types
   * @default empty
   */
  'credential-alias'?: { [key: string_creditial_alias_name]: string | string[] }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#doc
   */
  doc?: {
    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#docbrowser
     *
     * @default $BROWSER
     */
    browser?: string | string[]
  }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#cargo-new
   */
  'cargo-new'?: {
    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#cargo-newname
     *
     * @deprecated
     */
    name?: never

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#cargo-newemail
     *
     * @deprecated
     */
    email?: never

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#cargo-newvcs
     */
    vcs?: 'git' | 'hg' | 'pijul' | 'fossil' | 'none'
  }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#env
   *
   * @todo lack of official doc
   */
  env?: {
    [key: string]:
      | string
      | {
        value: string
        force?: boolean
        relative?: boolean
      }
  }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#future-incompat-report
   */
  'future-incompat-report'?: {
    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#future-incompat-reportfrequency
     *
     * @default 'always'
     */
    frequency?: 'always' | 'never'
  }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#http
   */
  http?: {
    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#httpdebug
     *
     * @default false
     */
    debug?: boolean

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#httpproxy
     *
     * @default undefined
     */
    proxy?: string_libcurl_proxy

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#httptimeout
     *
     * @default 30
     */
    timeout?: number

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#httpcainfo
     *
     * @default undefined
     */
    cainfo?: string

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#httpcheck-revoke
     *
     * @default windows_true_others_false
     */
    'check-revoke'?: boolean

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#httpssl-version
     *
     * @default undefined
     */
    'ssl-version'?:
      | string_tls_version
      | {
        max: string_tls_version
        min: string_tls_version
      }

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#httplow-speed-limit
     *
     * @default 10
     */
    'low-speed-limit'?: number

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#httpmultiplexing
     *
     * @default true
     */
    multiplexing?: boolean

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#httpuser-agent
     *
     * @default cargo_version
     */
    'user-agent'?: string
  }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#install
   */
  install?: {
    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#installroot
     *
     * @default cargo_home_directory
     */
    root?: string
  }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#net
   */
  net?: {
    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#netretry
     *
     * @default 3
     */
    retry?: number

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#netgit-fetch-with-cli
     *
     * @default false
     */
    'git-fetch-with-cli'?: boolean

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#netoffline
     *
     * @default false
     */
    offline?: boolean

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#netssh
     */
    ssh?: {
      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#netsshknown-hosts
       *
       * @default see_description
       */
      'known-hosts'?: string[]
    }
  }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#patch
   *
   * @todo
   */
  patch?: { [key: string_patch_name]: unknown }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#profile
   */
  profile?: {
    [key: string_profile_name]: {
      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenamebuild-override
       *
       * @todo
       */
      'build-override'?: { [key: string_profile_name]: unknown }

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenamepackagename
       *
       * @todo
       */
      package?: { [key: string_package_name]: unknown }

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenamecodegen-units
       *
       * https://doc.rust-lang.org/cargo/reference/profiles.html#codegen-units
       *
       * https://doc.rust-lang.org/rustc/codegen-options/index.html#codegen-units
       *
       * @default 256 | 16
       */
      'codegen-units'?: number

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenamedebug
       *
       * https://doc.rust-lang.org/cargo/reference/profiles.html#debug
       *
       * https://doc.rust-lang.org/rustc/codegen-options/index.html#debuginfo
       *
       * @default see_profile_docs
       */
      debug?:
        | 0
        | false
        | 'none'
        | 'line-directives-only'
        | 'line-tables-only'
        | 1
        | 'limited'
        | 2
        | true
        | 'full'

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenamesplit-debuginfo
       *
       * https://doc.rust-lang.org/cargo/reference/profiles.html#split-debuginfo
       *
       * https://doc.rust-lang.org/rustc/codegen-options/index.html#split-debuginfo
       *
       * @default see_profile_docs
       */
      'split-debuginfo'?: 'off' | 'packed' | 'unpacked'

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenamestrip
       *
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenamestrip-1
       *
       * https://doc.rust-lang.org/cargo/reference/profiles.html#strip
       *
       * https://doc.rust-lang.org/rustc/codegen-options/index.html#strip
       *
       * @default see_profile_docs
       *
       * @todo duplicated url
       */
      strip?: 'none' | 'debuginfo' | 'symbols' | true | false

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenamedebug-assertions
       *
       * https://doc.rust-lang.org/cargo/reference/profiles.html#debug-assertions
       *
       * https://doc.rust-lang.org/rustc/codegen-options/index.html#debug-assertions
       *
       * @default see_profile_docs
       */
      'debug-assertions'?: boolean

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenameincremental
       *
       * https://doc.rust-lang.org/cargo/reference/profiles.html#incremental
       *
       * https://doc.rust-lang.org/rustc/codegen-options/index.html#incremental
       *
       * @default see_profile_docs
       */
      incremental?: boolean

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenamelto
       *
       * https://doc.rust-lang.org/cargo/reference/profiles.html#lto
       *
       * https://doc.rust-lang.org/rustc/codegen-options/index.html#lto
       *
       * @default see_profile_docs
       */
      lto?: false | true | 'fat' | 'thin' | 'off'

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenameoverflow-checks
       *
       * https://doc.rust-lang.org/cargo/reference/profiles.html#overflow-checks
       *
       * https://doc.rust-lang.org/rustc/codegen-options/index.html#overflow-checks
       *
       * @default see_profile_docs
       */
      'overflow-checks'?: boolean

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenameopt-level
       *
       * https://doc.rust-lang.org/cargo/reference/profiles.html#opt-level
       *
       * https://doc.rust-lang.org/rustc/codegen-options/index.html#opt-level
       *
       * @default 0
       */
      'opt-level'?: 0 | 1 | 2 | 3 | 's' | 'z'

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenamepanic
       *
       * https://doc.rust-lang.org/cargo/reference/profiles.html#panic
       *
       * https://doc.rust-lang.org/rustc/codegen-options/index.html#panic
       *
       * @default see_profile_docs
       */
      panic?: 'unwind' | 'abort'

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#profilenamerpath
       *
       * https://doc.rust-lang.org/cargo/reference/profiles.html#rpath
       *
       * https://doc.rust-lang.org/rustc/codegen-options/index.html#rpath
       *
       * @default see_profile_docs
       */
      rpath?: boolean

      /**
       * @todo not documented
       */
      inherits?: string_profile_name
    }
  }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#registries
   */
  registries?: {
    [key: string_registry_name]: {
      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#registriesnameindex
       *
       * @default undefined
       */
      index?: string

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#registriesnametoken
       *
       * @default undefined
       */
      token?: string

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#registriesnamecredential-provider
       *
       * @default undefined
       *
       * @todo not sure, just roughly impl types
       */
      'credential-provider'?: string[]

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#registriescrates-ioprotocol
       *
       * @default 'sparse'
       */
      protocol?: 'git' | 'sparse'
    }
  }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#registry
   */
  registry?: {
    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#registryindex
     *
     * @deprecated
     */
    index?: never

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#registrydefault
     *
     * @default 'crates-io'
     */
    default?: string

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#registrycredential-provider
     *
     * @default undefined
     *
     * @todo not sure, just roughly impl types
     */
    'credential-provider'?: string[]

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#registrytoken
     *
     * @default undefined
     */
    token?: string

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#registryglobal-credential-providers
     *
     * @default 'cargo:token'
     */
    'global-credential-providers'?: string[]
  }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#source
   */
  source?: {
    [key: string]: {
      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#sourcenamereplace-with
       *
       * @default undefined
       */
      'replace-with'?: string

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#sourcenamedirectory
       *
       * @default undefined
       */
      directory?: string_path

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#sourcenameregistry
       *
       * @default undefined
       */
      registry?: string_url

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#sourcenamelocal-registry
       *
       * @default undefined
       */
      'local-registry'?: string_path

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#sourcenamegit
       *
       * @default undefined
       */
      git?: string_url

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#sourcenamebranch
       *
       * @default undefined
       */
      branch?: string

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#sourcenametag
       *
       * @default undefined
       */
      tag?: string

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#sourcenamerev
       *
       * @default undefined
       */
      rev?: string
    }
  }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#target
   */
  target?: {
    [key: string_triple | string_cfg]: {
      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#targettriplear
       *
       * @deprecated
       */
      ar?: never

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#targettriplelinker
       *
       * https://doc.rust-lang.org/cargo/reference/config.html#targetcfglinker
       *
       * @default undefined
       */
      linker?: string_program_path

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#targettriplerunner
       *
       * https://doc.rust-lang.org/cargo/reference/config.html#targetcfgrunner
       *
       * @default undefined
       */
      runner?: string | string[]

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#targettriplerustflags
       *
       * https://doc.rust-lang.org/cargo/reference/config.html#targetcfgrustflags
       *
       * @default undefined
       */
      rustflags?: string | string[]

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#targettriplerustdocflags
       */
      rustdocflags?: string | string[]
      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#targettriplelinks
       *
       * @todo target.<triple>.<links>
       */
    }
  }

  /**
   * https://doc.rust-lang.org/cargo/reference/config.html#term
   */
  term?: {
    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#termquiet
     *
     * @default false
     */
    quiet?: boolean

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#termverbose
     *
     * @default false
     */
    verbose?: boolean

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#termcolor
     *
     * @default "auto"
     */
    color?: 'auto' | 'always' | 'never'

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#termhyperlinks
     *
     * @default auto_detect
     */
    hyperlinks?: boolean

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#termunicode
     *
     * @default auto_detect
     */
    unicode?: boolean

    /**
     * https://doc.rust-lang.org/cargo/reference/config.html#termprogresswhen
     */
    progress?: {
      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#termprogresswhen
       *
       * @default 'auto'
       */
      when?: 'auto' | 'always' | 'never'

      /**
       * https://doc.rust-lang.org/cargo/reference/config.html#termprogresswidth
       *
       * @default undefined
       */
      width?: number
    }
  }
}

export type { CargoConfigSchema }
