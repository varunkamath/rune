// Auto-approve build scripts for these packages
module.exports = {
  hooks: {
    readPackage(pkg) {
      // Allow esbuild and unrs-resolver to run build scripts
      if (pkg.name === 'esbuild' || pkg.name === 'unrs-resolver') {
        pkg.trustedDependencies = ['esbuild', 'unrs-resolver'];
      }
      return pkg;
    }
  }
};