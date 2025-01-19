
# sentinel files are marker fils in your ~/.sentinel directory
# that we use to track whether a given tool that we need has been installed.
# You can always remove the ~/.sentinel directory which will simply cause
# the tools to be reinstalled. It also does not take any space since each
# sentinel file is a zero-byte file.
export ruby_version := env_var_or_default('ruby_version', "3.3.6")

sentinel_dir := join(home_dir, ".sentinel")
sentinel_ruby_install_installed := join(sentinel_dir, "ruby-install-installed")
sentinel_ruby_installed := join(sentinel_dir, "ruby-${ruby_version}-installed")
sentinel_jekyll_installed := join(sentinel_dir, "jekyll-installed")
sentinel_gem_bundle_installed := join(sentinel_dir, "gem-bundle-installed")
sentinel_gem_upgraded := join(sentinel_dir, "gem-upgraded")

_mkdir_sentinel_dir:
  #!/usr/bin/env bash
  test -d {{sentinel_dir}} || mkdir -p {{sentinel_dir}}

_path_exists name path:
  #!/usr/bin/env bash
  if test -e {{path}}; then
    echo "{{name}}: {{path}} exists"
  else
    echo "{{name}}: {{path}} does not exist"
  fi

sentinel-info: \
  (_path_exists "sentinel_dir" sentinel_dir) \
  (_path_exists "sentinel_ruby_install_installed" sentinel_ruby_install_installed) \
  (_path_exists "sentinel_ruby_installed" sentinel_ruby_installed) \
  (_path_exists "sentinel_jekyll_installed" sentinel_jekyll_installed) \
  (_path_exists "sentinel_gem_bundle_installed" sentinel_gem_bundle_installed) \
  (_path_exists "sentinel_gem_upgraded" sentinel_gem_upgraded)

sentinel-clean:
  #!/usr/bin/env bash
  rm -rf {{sentinel_dir}}
