library 'jenkins_lib@main'

pipeline {
    agent any
    options {
        ansiColor('xterm')
    }
    environment {
        // AWS_ECR_URL needs to be set in jenkin's config.
        // AWS_ECR_URL could really be any docker registry. we just use ECR so that we don't have to manage it
        REPO_NAME="web3-this-then-that"

        // branch that should get tagged with "latest_$arch" (stable, main, master, etc.)
        LATEST_BRANCH="main"
    }
    stages {
        stage('build and push') {
            parallel {
                stage('build and push arm64 image') {
                    agent {
                        label 'arm64'
                    }
                    environment {
                        ARCH="arm64"
                    }
                    steps {
                        script {
                            myBuildandPush.buildAndPush()
                        }
                    }
                }
            }

        }
        stage('push latest') {
            parallel {
                stage('maybe push latest_arm64 tag') {
                    agent any
                    environment {
                        ARCH="arm64"
                    }
                    steps {
                        script {
                            myPushLatest.maybePushLatest()
                        }
                    }
                }
            }
        }
    }
}