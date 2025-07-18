pipeline {
    agent any
    stages {
        stage("Git Checkout Stage") {
            steps {
                git branch: 'testing', url: 'https://github.com/rajeshrj-git/clippy.git'
                echo 'Checkout Successful ✅'
            }
        }
        
        // stage('Copy .env file') {
        //     steps {
        //         script {
        //             withCredentials([file(credentialsId: 'clippy-env-file', variable: 'ENV_FILE')]) {
        //                 sh 'cp $ENV_FILE .env'
        //             }
        //         }
        //         echo '.env file copied successfully ✅'
        //     }
        // }
        
        stage('Docker image Build and Docker compose Up') {
            steps {
                sh 'docker-compose up --build -d'
                echo 'Docker compose Up Successfully ✅'
            }
        }
    }
}