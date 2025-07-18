pipeline{

    agent any

    stages{
        stage("Git Checkout Stage"){
            steps{
                git branch: 'testing', url: 'https://github.com/rajeshrj-git/clippy.git'
                echo 'Checkout Successfull ✅'
                }
        }
         stage('Copy .env file') {
            steps {
                script {
                    // Option 1: Copy from Jenkins home directory
                    sh 'cp /var/lib/jenkins/env-files/clippy/.env .'
                    
                    // Option 2: Use Jenkins credentials (secret file)
                    // withCredentials([file(credentialsId: 'clippy-env-file', variable: 'ENV_FILE')]) {
                    //     sh 'cp $ENV_FILE .env'
                    // }
                }
                echo '.env file copied successfully ✅'
            }
        }
        stage('Docker image Build  and Docker compse Up '){
            steps{
                sh 'docker-compose up --build -d'
                echo 'Docker compse Up Successfully ✅'
            }
        }
    }
}